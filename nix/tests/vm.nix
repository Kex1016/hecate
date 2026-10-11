# NixOS VM test: install hecate with the module, start it headless as a user, check that a
# Wayland client sees the expected globals, and that config validation works.
{ self, pkgs }:
pkgs.testers.runNixOSTest {
  name = "hecate-vm";
  nodes.machine =
    { pkgs, ... }:
    {
      imports = [ self.nixosModules.default ];
      programs.hecate.enable = true;
      users.users.alice = {
        isNormalUser = true;
        uid = 1000;
      };
      environment.systemPackages = [
        pkgs.wayland-utils
        pkgs.quickshell
      ];
      # A plain QuickShell bar: one layer-shell PanelWindow with an exclusive zone.
      environment.etc."qs-test/shell.qml".text = ''
        import Quickshell
        import QtQuick

        PanelWindow {
          anchors { top: true; left: true; right: true }
          implicitHeight: 30
          Text { text: "hecate" }
        }
      '';
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
    machine.wait_for_unit("multi-user.target")
    machine.succeed("loginctl enable-linger alice")
    machine.wait_for_unit("user@1000.service")
    # `su -` gives no session environment; the user manager's bus lives in /run/user/1000.
    def as_alice(cmd):
        return f"su - alice -c 'export XDG_RUNTIME_DIR=/run/user/1000; {cmd}'"

    machine.succeed(as_alice("systemctl --user start hecate-headless"))
    machine.wait_until_succeeds("ls /run/user/1000/wayland-*", timeout=60)

    info = machine.succeed(
        as_alice("WAYLAND_DISPLAY=$(basename $(ls /run/user/1000/wayland-? | head -1)) wayland-info")
    )
    for wanted in [
        "xdg_wm_base",
        "zwlr_layer_shell_v1",
        "ext_session_lock_manager_v1",
        "zxdg_decoration_manager_v1",
        "wp_fractional_scale_manager_v1",
        "zwp_pointer_constraints_v1",
        "zwp_relative_pointer_manager_v1",
        "ext_idle_notifier_v1",
        "zwp_idle_inhibit_manager_v1",
        "zwlr_data_control_manager_v1",
        "zwlr_foreign_toplevel_manager_v1",
        "ext_foreign_toplevel_list_v1",
        "ext_workspace_manager_v1",
        "zwlr_screencopy_manager_v1",
        "ext_image_copy_capture_manager_v1",
        "zwlr_output_manager_v1",
        "zwlr_output_power_manager_v1",
        "zwlr_gamma_control_manager_v1",
        "hyprland_focus_grab_manager_v1",
    ]:
        assert wanted in info, f"missing global {wanted}"

    # QuickShell maps its bar as a layer surface.
    sock = "$(ls /run/user/1000/hecate.*.sock | head -1)"
    disp = "$(basename $(ls /run/user/1000/wayland-? | head -1))"
    machine.succeed(as_alice(
        f"WAYLAND_DISPLAY={disp} QT_QPA_PLATFORM=wayland QT_QUICK_BACKEND=software "
        "systemd-run --user --unit qs-test quickshell -p /etc/qs-test/shell.qml"
    ))
    machine.wait_until_succeeds(
        as_alice(f"HECATE_SOCKET={sock} hecate msg --json layers | grep -q quickshell"), timeout=90
    )

    machine.succeed("hecate validate /dev/null || true")
    machine.succeed("echo '[workspaces]\ncount = 4' > /tmp/ok.toml && hecate validate /tmp/ok.toml")
    machine.fail("echo '[workspaces]\ncount = 99' > /tmp/bad.toml && hecate validate /tmp/bad.toml")
    machine.succeed("test -f /run/current-system/sw/share/wayland-sessions/hecate.desktop")
  '';
}
