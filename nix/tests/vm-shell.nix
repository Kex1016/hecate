# NixOS VM test for the bundled shell: alice's home comes from the Home Manager module with the
# shell on. hecate runs headless, the shell's own user unit is started against it, and the test
# checks that the shell's layers map, its shortcuts register, and its binds reach hecate. It then
# loads the desktop with every widget on, once in each widget style (free, then windows, then free
# again by switching the setting under the running shell), and fails on any QML problem a widget
# logs (widget-log.py says which lines count).
{
  self,
  pkgs,
  home-manager,
}:
let
  # Every widget the loader knows, from the catalogue itself.
  widgetKeys = map builtins.head (
    builtins.filter builtins.isList (
      builtins.split ''\{ key: "([A-Za-z0-9]+)"'' (
        builtins.readFile ../../shell/end4-pC/services/DesktopWidgets.qml
      )
    )
  );
  # desktop-widgets.json: all of them on (the defaults would show only a few of them).
  widgetState = pkgs.writeText "desktop-widgets.json" (
    builtins.toJSON { enabled = builtins.listToAttrs (map (k: { name = k; value = true; }) widgetKeys); }
  );
  styleConfig =
    form: pkgs.writeText "config-${form}.json" (builtins.toJSON { background.widgets.form = form; });
in
assert builtins.length widgetKeys >= 28;
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

        # --- every widget, in each style ---------------------------------------------------
        import importlib.util

        spec = importlib.util.spec_from_file_location("widget_log", "${./widget-log.py}")
        assert spec is not None and spec.loader is not None
        widget_log = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(widget_log)

        configs = {"free": "${styleConfig "free"}", "window": "${styleConfig "window"}"}
        alice_config = "/home/alice/.config/illogical-impulse/config.json"
        alice_state = "/home/alice/.local/state/quickshell/user/desktop-widgets.json"

        def journal_cursor():
            out = machine.succeed(as_alice("journalctl --user -u hecate-shell --no-pager -n 0 --show-cursor"))
            return out.split("cursor:")[-1].strip()

        def set_style(style):
            machine.succeed(f"install -o alice -m 644 {configs[style]} {alice_config}")

        def check_widgets(style, cursor):
            # The loader reads the style from the config file the shell watches, so what it
            # shows is what the file says; the journal since `cursor` is what the widgets said.
            machine.succeed(f"grep -Eq '\"form\": *\"{style}\"' {alice_config}")
            machine.succeed(as_alice("systemctl --user is-active hecate-shell"))
            log = machine.succeed(as_alice(f"journalctl --user -u hecate-shell --no-pager --after-cursor={cursor}"))
            problems = widget_log.widget_problems(log)
            assert not problems, f"widgets logged problems in the {style} style:\n" + "\n".join(problems)

        # Loaded: the widgets slots settle a few seconds after the background maps (the loader
        # first lists which free files exist, then each widget loads its file).
        settle = 30

        for style in ("free", "window"):
            machine.succeed(as_alice("systemctl --user stop hecate-shell"))
            machine.succeed(as_alice("mkdir -p ~/.config/illogical-impulse ~/.local/state/quickshell/user"))
            set_style(style)
            machine.succeed(f"install -o alice -m 644 ${widgetState} {alice_state}")
            cursor = journal_cursor()
            machine.succeed(as_alice("systemctl --user start hecate-shell"))
            machine.wait_until_succeeds(has_layer("background"), timeout=180)
            machine.sleep(settle)
            check_widgets(style, cursor)

        # Windows to free under the running shell: every widget swaps its form live.
        cursor = journal_cursor()
        set_style("free")
        machine.sleep(settle)
        check_widgets("free", cursor)
    except Exception:
        dump()
        raise
  '';
}
