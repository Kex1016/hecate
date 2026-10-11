# NixOS / flakes

First-class target: install, run and fully configure hecate **and the
bundled shell** on NixOS from this repo's flake, with config written as plain
Nix attrsets.

## flake.nix outputs

```
packages.<system>.default        hecate (compositor + `msg` CLI)
packages.<system>.hecate-shell   bundled end4-pC shell, wrapped with QuickShell, Qt modules, tools, Python env
overlays.default                 pkgs.hecate, pkgs.hecate-shell
nixosModules.default             programs.hecate
homeManagerModules.default       wayland.windowManager.hecate
devShells.<system>.default       rust toolchain, rust-analyzer, deps, wayland-info, foot, xwayland-satellite
checks.<system>.*                build, clippy, fmt, tests, example-config validation, nixos VM test
formatter.<system>               nixfmt-rfc-style
```

Systems: `x86_64-linux`, `aarch64-linux`.

Inputs: `nixpkgs` (unstable), `crane` (incremental Rust builds, cached
deps) and `quickshell` (the shell tracks quickshell-git, so we pin the
flake input rather than nixpkgs' release). The shell sources aren't
inputs: they're vendored under `shell/` (see [shell.md](shell.md#repo-layout)). `rust-overlay` only if we need a newer toolchain than nixpkgs ships.
No flake-utils; a small `forAllSystems` helper is enough.

## Package

- `nativeBuildInputs`: `pkg-config`, `makeWrapper`.
- `buildInputs`: `libinput`, `libxkbcommon`, `libgbm`, `libdrm`, `udev`,
  `seatd` (libseat), `wayland`, `pixman`, `libdisplay-info`.
- Wrapped with `--prefix PATH : ${xwayland-satellite}/bin` (satellite
  itself brings `xwayland`). EGL/GBM drivers come from
  `/run/opengl-driver` via `hardware.graphics`.
- Installs `share/wayland-sessions/hecate.desktop`,
  `share/xdg-desktop-portal/hecate-portals.conf` and the JSON Schema;
  `passthru.providedSessions = [ "hecate" ]`.
- Installs shell completions (bash, zsh, fish) and man pages generated at
  build time (`hecate completions`, clap_mangen).
- `meta.mainProgram = "hecate"`, `meta.license = lib.licenses.gpl3Plus`.

## NixOS module: `programs.hecate`

```nix
{
  imports = [ inputs.hecate.nixosModules.default ];
  programs.hecate.enable = true;
}
```

Does:

- `environment.systemPackages = [ cfg.package ]`
- `services.displayManager.sessionPackages = [ cfg.package ]`
- `hardware.graphics.enable = true`
- `security.polkit.enable = true`, `programs.dconf.enable = true`
- `xdg.portal`: enable, `extraPortals = [ xdg-desktop-portal-wlr xdg-desktop-portal-gtk ]`,
  `configPackages = [ cfg.package ]`. The shipped `hecate-portals.conf`:
  `default=gtk`, `ScreenCast=wlr`, `Screenshot=wlr`,
  `Secret=gnome-keyring`.
  (`xdg-desktop-portal-gnome` won't work: it needs Mutter's D-Bus
  screencast API, which niri emulates and we don't.)
- Optional `programs.hecate.shell.enable`: system-side needs of the
  bundled shell (PAM service for its lock screen, geoclue2, i2c for ddcutil,
  upower, NetworkManager, ydotool, fonts). Pairs with the HM option.
- Optional `programs.hecate.gaming.enable`: `hardware.graphics.enable32Bit`,
  `programs.gamescope.enable`. Kept small; you already configure Steam and
  gamemode in `generic/system/gaming.nix`.

## Home Manager module: `wayland.windowManager.hecate`

```nix
{
  wayland.windowManager.hecate = {
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
- `systemd.enable` (default true): creates `hecate-session.target`
  (`BindsTo=graphical-session.target`, `Wants=graphical-session-pre.target`)
  so shells and daemons run as user services (the bundled shell does this
  automatically, see below).
- `systemd.variables`: env vars imported into the activation environment
  at startup (default: `WAYLAND_DISPLAY DISPLAY XDG_CURRENT_DESKTOP
  XDG_SESSION_TYPE HECATE_SOCKET NIXOS_OZONE_WL`).

Does:

- Writes `xdg.configFile."hecate/config.toml"` with `pkgs.formats.toml`.
- Validates it at build time: the generated file is passed through
  `hecate validate`, so a typo fails `home-manager switch` / `nixos-rebuild`
  instead of breaking login.

### Typing

v1 uses the freeform TOML type plus build-time validation (cheap, always in
sync with the compositor). After v1, generate fully typed Nix options with
descriptions from the JSON Schema, like niri-flake's typed `settings`,
so `nixos-option` and editor completion work in Nix too.

## Bundled shell

`wayland.windowManager.hecate.shell`:

```nix
{
  wayland.windowManager.hecate.shell = {
    enable = true;                 # default false
    package = pkgs.hecate-shell;   # from the overlay
    keybinds = true;               # add the end-4 profile as `shell_binds` (keybinds.md)
    dotfiles = [ "matugen" "kde-material-you-colors" "Kvantum" "fuzzel" ];  # default; also fontconfig, kitty, foot, fish
    extraPackages = [ ];           # more tools on the shell's PATH (hecate itself is always there)
  };
}
```

On the NixOS side, `programs.hecate.shell.enable = true` turns on what the shell needs from
the system (see below).

What `hecate-shell` is (`nix/shell-package.nix`):

- `share/hecate-shell/end4-pC/`: the patched QML tree from `shell/end4-pC`,
  read-only in the store. The Nix build disables the self-updater.
- `bin/hecate-shell`: runs `qs -c end4-pC` (the HM module links the config into
  `~/.config/quickshell`) or `qs -p <store path>`; `bin/hecate-shell-ipc` calls into
  the running shell. Both go through a wrapper built from
  dots-hyprland's `sdata/dist-nix/home-manager/quickshell.nix`:
  `wrapQtAppsHook` with qt5compat, qtdeclarative, qtimageformats,
  qtmultimedia, qtpositioning, qtquicktimeline, qtsensors, qtsvg,
  qtvirtualkeyboard, qtwayland, kirigami, syntax-highlighting.
- Runtime `PATH` from `sdata/deps-info.md`: matugen, cliphist, wl-clipboard,
  grim, slurp, wf-recorder, tesseract, translate-shell, jq, curl, wget, bc,
  brightnessctl, ddcutil, playerctl, cava, pavucontrol-qt, wlsunset,
  hyprpicker, fuzzel, ydotool, libnotify, xdg-user-dirs, geoclue2 demo
  agent, kde-material-you-colors, plus `hecate` itself for `hecate msg`.
- Python env from `sdata/uv/requirements.in`, all from nixpkgs, behind a small
  `bin/activate` so the shell's `$ILLOGICAL_IMPULSE_VIRTUAL_ENV` scripts work unchanged.
- Fonts (`fonts.packages` hint in the NixOS module, `home.packages` in HM):
  Material Symbols, Space Grotesk, Rubik, JetBrains Mono Nerd, Readex Pro,
  and the rest from the `illogical-impulse-fonts-themes` list.
- Themes: adw-gtk3, Breeze, Darkly (Qt), Bibata cursor.

What the HM module does with it:

- `systemd.user.services.hecate-shell`: `ExecStart=hecate-shell`,
  `PartOf`/`WantedBy=hecate-session.target`, restart on failure.
- Adds the shell keybind profile to the generated hecate config (your own
  `settings.binds` still win).
- Adds `include = [ "shell.toml" ]` so the shell's settings UI can write
  compositor tweaks to `~/.config/hecate/shell.toml` without touching the
  Nix-made file.
- Installs the dotfiles above into `~/.config` (matugen writes generated
  colours into mutable locations, so templates are linked and outputs are
  left writable).
- Leaves mutable shell state alone: `~/.config/illogical-impulse/`
  (`config.json`, written by the settings UI), `~/.local/state/quickshell`,
  `~/.cache/quickshell`.
- Needs from the NixOS side: PAM entry for the QuickShell lock screen
  (`security.pam.services.quickshell`), `services.geoclue2`, `hardware.i2c` for
  ddcutil, `services.upower`, `networking.networkmanager`, `hardware.bluetooth`,
  ydotool and gnome-keyring. The NixOS module's `programs.hecate.shell.enable` turns
  these on (as defaults you can override).

## Other shells

- Plain QuickShell configs: `pkgs.quickshell` or its flake, started from
  `autostart` or as a user service bound to `graphical-session.target`.
- Noctalia: same; works through protocols (see PLAN.md section 6).

## VM test (checks)

`nixosTest` with virtio-gpu: log in a user, start hecate on the DRM backend
(or the headless backend if virtio-gpu proves flaky in CI), wait for the
socket, run `wayland-info` and assert the expected globals are present,
`hecate msg outputs`, start an X11 client to exercise satellite, start
`hecate-shell`, check its bar's layer surface exists (`hecate msg -j
layers`), take a screenshot with `grim`.
