# NixOS / flakes

First-class target: install and run wl-hex on NixOS from this repo's flake
with no extra overlays or manual steps.

## flake.nix outputs

```
packages.<system>.default        wl-hex (compositor + wl-hex msg CLI later)
packages.<system>.wl-hex
overlays.default                 pkgs.wl-hex
nixosModules.default             programs.wl-hex
homeManagerModules.default       wayland.windowManager.wl-hex
devShells.<system>.default       rust toolchain, rust-analyzer, deps, wayland-info, weston-* clients
checks.<system>.*                build, clippy, fmt, tests, nixos VM test
formatter.<system>               nixfmt-rfc-style
```

Systems: `x86_64-linux`, `aarch64-linux`.

Inputs: `nixpkgs` (unstable), `crane` (incremental Rust builds, cached deps),
`rust-overlay` or `fenix` only if we need a newer toolchain than nixpkgs
ships. Keep inputs minimal; no flake-utils if a small `forAllSystems`
helper does the job.

## Package

- `nativeBuildInputs`: `pkg-config`, `rustPlatform.bindgenHook` (if needed).
- `buildInputs`: `libinput`, `libxkbcommon`, `mesa`/`libgbm`, `libdrm`,
  `udev`, `seatd` (libseat), `wayland`, `pixman`, `libdisplay-info`,
  `lua5_4` (if `mlua` uses system Lua, otherwise vendored).
- Runtime: `xwayland` on `PATH` (wrapped with `makeWrapper --prefix PATH`),
  EGL/GBM drivers come from `/run/opengl-driver` via `hardware.graphics`.
- Installs `share/wayland-sessions/wl-hex.desktop` and
  `share/xdg-desktop-portal/wl-hex-portals.conf`;
  `passthru.providedSessions = [ "wl-hex" ]`.
- `meta.mainProgram = "wl-hex"`.

## NixOS module: `programs.wl-hex`

```nix
{
  programs.wl-hex = {
    enable = true;
    package = inputs.wl-hex.packages.${pkgs.system}.default;
    xwayland.enable = true;
  };
}
```

Does:

- `environment.systemPackages = [ cfg.package ]`
- `services.displayManager.sessionPackages = [ cfg.package ]`
- `hardware.graphics.enable = true`
- `security.polkit.enable = true`, `programs.dconf.enable = true`
- `xdg.portal`: enable, `extraPortals = [ xdg-desktop-portal-wlr xdg-desktop-portal-gtk ]`,
  `configPackages = [ cfg.package ]` (ships `wl-hex-portals.conf`:
  ScreenCast/Screenshot → wlr, everything else → gtk)
- `services.seatd` not required when logind is present (libseat uses logind)
- `programs.xwayland.enable = cfg.xwayland.enable`
- Optional `programs.wl-hex.gaming.enable`: turns on
  `hardware.graphics.enable32Bit`, adds `gamescope`, sets
  `programs.steam.extraCompatPackages` hints. Kept small; users usually
  configure Steam themselves.

## Home Manager module: `wayland.windowManager.wl-hex`

```nix
{
  wayland.windowManager.wl-hex = {
    enable = true;
    format = "lua";                  # or "json"
    settings = {
      border = {
        width = 2;
        radius = { top_left = 12; top_right = 12; bottom_right = 4; bottom_left = 4; };
        color = { active = "#89b4fa"; inactive = "#45475a"; };
      };
      keyboard.binds = [
        { key = "Super+Return"; action.spawn = [ "foot" ]; }
      ];
    };
    extraConfig = ''
      -- raw Lua appended; can modify `config` before it is returned
    '';
    systemd.enable = true;
  };
}
```

Does:

- Writes `xdg.configFile."wl-hex/init.lua"` (via `lib.generators.toLua`)
  or `config.json`.
- `systemd.enable` creates `wl-hex-session.target` (`BindsTo` +
  `Wants=graphical-session-pre.target`, `Before=graphical-session.target`),
  so QuickShell can run as a user service:
  `systemd.user.services.quickshell.Install.WantedBy = [ "graphical-session.target" ]`.
- The compositor, on start, runs `dbus-update-activation-environment
  --systemd WAYLAND_DISPLAY DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_TYPE`
  and starts the target; on exit stops it. (labwc does the same in
  `config/session.c`.)
- Config validation at build time: `checkPhase` runs
  `wl-hex --check-config <file>` so a bad config fails `home-manager switch`
  instead of at login.

## QuickShell on NixOS

QuickShell is in nixpkgs (`pkgs.quickshell`) and has its own flake. We
document both:

- simple: `autostart = { { "qs" } }` in config
- systemd: HM user service bound to `graphical-session.target`

## VM test (checks)

`nixosTest` with `virtualisation.qemu.options = [ "-vga none" "-device virtio-gpu-pci" ]`:
log in a user, start `wl-hex` on the DRM backend (or headless backend if
virtio-gpu proves flaky in CI), wait for the socket, run `wayland-info` and
assert the expected globals are present, start a QuickShell config with a
`PanelWindow`, take a screenshot via `grim`.
