# NixOS / flakes

First-class target: install, run and fully configure wl-hex on NixOS from
this repo's flake, with config written as plain Nix attrsets.

## flake.nix outputs

```
packages.<system>.default        wl-hex (compositor + `msg` CLI)
overlays.default                 pkgs.wl-hex
nixosModules.default             programs.wl-hex
homeManagerModules.default       wayland.windowManager.wl-hex
devShells.<system>.default       rust toolchain, rust-analyzer, deps, wayland-info, foot, xwayland-satellite
checks.<system>.*                build, clippy, fmt, tests, example-config validation, nixos VM test
formatter.<system>               nixfmt-rfc-style
```

Systems: `x86_64-linux`, `aarch64-linux`.

Inputs: `nixpkgs` (unstable) and `crane` (incremental Rust builds, cached
deps). `rust-overlay` only if we need a newer toolchain than nixpkgs ships.
No flake-utils; a small `forAllSystems` helper is enough.

## Package

- `nativeBuildInputs`: `pkg-config`, `makeWrapper`.
- `buildInputs`: `libinput`, `libxkbcommon`, `libgbm`, `libdrm`, `udev`,
  `seatd` (libseat), `wayland`, `pixman`, `libdisplay-info`.
- Wrapped with `--prefix PATH : ${xwayland-satellite}/bin` (satellite
  itself brings `xwayland`). EGL/GBM drivers come from
  `/run/opengl-driver` via `hardware.graphics`.
- Installs `share/wayland-sessions/wl-hex.desktop`,
  `share/xdg-desktop-portal/wl-hex-portals.conf` and the JSON Schema;
  `passthru.providedSessions = [ "wl-hex" ]`.
- `meta.mainProgram = "wl-hex"`, `meta.license = lib.licenses.gpl3Plus`.

## NixOS module: `programs.wl-hex`

```nix
{
  imports = [ inputs.wl-hex.nixosModules.default ];
  programs.wl-hex.enable = true;
}
```

Does:

- `environment.systemPackages = [ cfg.package ]`
- `services.displayManager.sessionPackages = [ cfg.package ]`
- `hardware.graphics.enable = true`
- `security.polkit.enable = true`, `programs.dconf.enable = true`
- `xdg.portal`: enable, `extraPortals = [ xdg-desktop-portal-wlr xdg-desktop-portal-gtk ]`,
  `configPackages = [ cfg.package ]`. The shipped `wl-hex-portals.conf`:
  `default=gtk`, `ScreenCast=wlr`, `Screenshot=wlr`,
  `Secret=gnome-keyring`.
  (`xdg-desktop-portal-gnome` won't work: it needs Mutter's D-Bus
  screencast API, which niri emulates and we don't.)
- Optional `programs.wl-hex.gaming.enable`: `hardware.graphics.enable32Bit`,
  `programs.gamescope.enable`. Kept small; you already configure Steam and
  gamemode in `generic/system/gaming.nix`.

## Home Manager module: `wayland.windowManager.wl-hex`

```nix
{
  wayland.windowManager.wl-hex = {
    enable = true;
    settings = { /* plain attrset, see config.md */ };
    systemd.enable = true;
  };
}
```

Options:

- `settings`: type `(pkgs.formats.toml { }).type`, i.e. any attrset that
  serialises to TOML. Merges across modules like any NixOS attrset, so a
  host module can override or add binds from a shared module.
- `extraConfigFiles`: list of paths added to `include`.
- `package`: used for validation and `systemd` paths.
- `systemd.enable` (default true): creates `wl-hex-session.target`
  (`BindsTo=graphical-session.target`, `Wants=graphical-session-pre.target`)
  so shells and daemons run as user services, e.g.
  `systemd.user.services.noctalia-shell.Install.WantedBy = [ "graphical-session.target" ]`.
- `systemd.variables`: env vars imported into the activation environment
  at startup (default: `WAYLAND_DISPLAY DISPLAY XDG_CURRENT_DESKTOP
  XDG_SESSION_TYPE WL_HEX_SOCKET NIXOS_OZONE_WL`).

Does:

- Writes `xdg.configFile."wl-hex/config.toml"` with `pkgs.formats.toml`.
- Validates it at build time: the generated file is passed through
  `wl-hex validate`, so a typo fails `home-manager switch` / `nixos-rebuild`
  instead of breaking login.

### Typing

v1 uses the freeform TOML type plus build-time validation (cheap, always in
sync with the compositor). After v1, generate fully typed Nix options with
descriptions from the JSON Schema, like niri-flake's typed `settings`,
so `nixos-option` and editor completion work in Nix too.

## Shells on NixOS

- Noctalia: your flake already pulls it. Run it from `autostart` or as a
  user service bound to `graphical-session.target`.
- QuickShell: `pkgs.quickshell` or its flake; same two options.

## VM test (checks)

`nixosTest` with virtio-gpu: log in a user, start wl-hex on the DRM backend
(or the headless backend if virtio-gpu proves flaky in CI), wait for the
socket, run `wayland-info` and assert the expected globals are present,
`wl-hex msg outputs`, start an X11 client to exercise satellite, start a
QuickShell config with a `PanelWindow`, take a screenshot with `grim`.
