# NixOS VM test for the bundled shell: alice's home comes from the Home Manager module with the
# shell on. hecate runs headless, the shell's own user unit is started against it, and the test
# checks that the shell's layers map, its shortcuts register, and its binds reach hecate.
{
  self,
  pkgs,
  home-manager,
}:
pkgs.testers.runNixOSTest {
  name = "hecate-vm-shell";
  nodes.machine =
    { pkgs, ... }:
    {
      imports = [
        self.nixosModules.default
        home-manager.nixosModules.home-manager
      ];
      virtualisation = {
        memorySize = 4096;
        cores = 2;
      };
      programs.hecate = {
        enable = true;
        shell.enable = true;
      };
      users.users.alice = {
        isNormalUser = true;
        uid = 1000;
      };
      home-manager.users.alice = {
        imports = [ self.homeManagerModules.default ];
        home.stateVersion = "25.11";
        wayland.windowManager.hecate = {
          enable = true;
          shell.enable = true;
        };
      };
      systemd.user.services.hecate-headless = {
        description = "hecate (headless, for testing)";
        environment = {
          HECATE_NO_SESSION = "1";
          HECATE_LOG = "hecate=debug";
        };
        serviceConfig.ExecStart = "${
          self.packages.${pkgs.stdenv.hostPlatform.system}.hecate
        }/bin/hecate --backend headless";
      };
    };

  testScript = ''
    import json

    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("home-manager-alice.service")
    machine.succeed("loginctl enable-linger alice")
    machine.wait_for_unit("user@1000.service")

    def as_alice(cmd):
        return f"su - alice -c 'export XDG_RUNTIME_DIR=/run/user/1000; {cmd}'"

    machine.succeed(as_alice("systemctl --user start hecate-headless"))
    machine.wait_until_succeeds("ls /run/user/1000/wayland-?", timeout=60)
    machine.wait_until_succeeds("ls /run/user/1000/hecate.*.sock", timeout=30)
    disp = machine.succeed("basename $(ls /run/user/1000/wayland-? | head -1)").strip()
    sock = machine.succeed("ls /run/user/1000/hecate.*.sock | head -1").strip()

    def msg(args):
        return machine.succeed(as_alice(f"HECATE_SOCKET={sock} hecate msg {args}"))

    # The config from the Home Manager module carries the shell's binds.
    binds = json.loads(msg("--json binds"))
    shell_binds = [b for b in binds if b.get("source") == "shell"]
    assert shell_binds, "no shell binds in the generated config"

    # Start the shell through its own unit, the way hecate-session.target would.
    # What hecate imports into the session on a real login (HECATE_NO_SESSION skips it here);
    # the shell tells it's on hecate from these.
    machine.succeed(as_alice(
        f"systemctl --user set-environment WAYLAND_DISPLAY={disp} HECATE_SOCKET={sock} "
        "XDG_CURRENT_DESKTOP=hecate QT_QUICK_BACKEND=software"
    ))
    machine.succeed(as_alice("systemctl --user start hecate-shell"))

    def dump():
        print(machine.execute(as_alice("journalctl --user -u hecate-shell --no-pager | grep -v icon | tail -80"))[1])
        print(machine.execute(as_alice(f"HECATE_SOCKET={sock} hecate msg --json layers"))[1])
        print(machine.execute(as_alice("timeout 15 qs list --all; echo qs-list-rc=$?"))[1])
        print(machine.execute(as_alice("timeout 15 hecate-shell-ipc shortcut list; echo ipc-rc=$?"))[1])

    try:
        # Count layers by namespace: transient ones (a toast, the OSD) come and go on their own.
        def has_layer(name):
            return as_alice(f"HECATE_SOCKET={sock} hecate msg --json layers | grep -qw quickshell:{name}")

        # The bar and the background both map as layer surfaces.
        machine.wait_until_succeeds(has_layer("background"), timeout=180)
        machine.wait_until_succeeds(has_layer("bar"), timeout=30)
        # Every shortcut the shell's binds trigger is registered over its IPC.
        machine.wait_until_succeeds(
            as_alice("timeout 15 hecate-shell-ipc shortcut list | grep -q sidebarRightToggle"), timeout=180
        )
        machine.fail(has_layer("sidebarRight"))
        machine.succeed(as_alice("timeout 15 hecate-shell-ipc shortcut trigger sidebarRightToggle"))
        machine.wait_until_succeeds(has_layer("sidebarRight"), timeout=30)
        machine.succeed(as_alice("systemctl --user is-active hecate-shell"))

        # Nothing the shell loaded may have failed to compile or bind: a component that cannot
        # load is dropped silently by its loader, so the journal is the only place it shows.
        # Double quotes: as_alice already wraps the command in single ones.
        # (Runtime tools the VM lacks, like a player or a network, are not QML errors, and
        # neither is the shell's own logging, such as "[To Do] File not found, creating new file.")
        qml_errors = (
            "is not a type|is not installed|Cannot assign|Cannot read property|"
            "ReferenceError|TypeError|Unable to assign|"
            "Type .* unavailable|File not found|Cannot open"
        )
        errors = machine.execute(
            as_alice(
                "journalctl --user -u hecate-shell --no-pager | grep -v icon "
                '| grep -v "qml: \\[" '
                f'| grep -E "{qml_errors}"'
            )
        )[1]
        assert not errors.strip(), "the shell logged QML errors:\n" + errors
    except Exception:
        dump()
        raise
  '';
}
