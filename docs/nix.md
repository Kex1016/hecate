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
checks.<system>.*                hecate, hecate-shell, clippy, fmt, test, hm (Home Manager build
                                 with the shell, config validated), keybind-docs, vm, vm-shell
formatter.<system>               nixfmt
```

Systems: `x86_64-linux`, `aarch64-linux`.

Inputs: `nixpkgs` (unstable; QuickShell comes from there), `crane` (incremental Rust
builds, cached deps) and `home-manager` (only for the `hm` and `vm-shell` checks; it follows
`nixpkgs`). The shell sources aren't inputs: they're vendored under `shell/` (see
[shell.md](shell.md#repo-layout)). No flake-utils; a small `forAllSystems` helper is enough.

## Package

- `nativeBuildInputs`: `pkg-config`, `makeWrapper`.
- `buildInputs`: `libinput`, `libxkbcommon`, `libgbm`, `libdrm`, `udev`,
  `seatd` (libseat), `wayland`, `pixman`, `libdisplay-info`.
- Wrapped with `--prefix PATH : ${xwayland-satellite}/bin` (satellite
  itself brings `xwayland`). EGL/GBM drivers come from
  `/run/opengl-driver` via `hardware.graphics`.
- Installs `share/wayland-sessions/hecate.desktop`,
  `share/xdg-desktop-portal/hecate-portals.conf`, `share/systemd/user/hecate-session.target`
  and the config's JSON Schema at `share/hecate/config.schema.json`;
  `passthru.providedSessions = [ "hecate" ]`.
- Installs shell completions (bash, zsh, fish) generated at build time (`hecate completions`).
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

## Haiiro fonts and hecate-mcp

- `packages.<system>.haiiro-fonts` (from `nix/fonts.nix`): IBM VGA 8x16,
  Jacquard 24 and GNU Unifont, with their fontconfig fallback order.
  `programs.hecate.fonts.enable` (NixOS) installs them system-wide. The Home
  Manager module adds them to the shell's font fallbacks.
- `packages.<system>.hecate-mcp`: the MCP server alone, with `grim` and
  `notify-send` on its `PATH`. See [hecate-mcp.md](hecate-mcp.md).
- `wayland.windowManager.hecate.shell.blobs.resume` (default
  `/run/hecate-greetd/blobs.json`): where the blob background carries on
  from at startup. `null` leaves it out.
- `wayland.windowManager.hecate.shell.theming.compositor`: the haiiro look
  for the compositor's frame and animations ([config.md](config.md#haiiro-look)).

## Bundled shell

`wayland.windowManager.hecate.shell`:

```nix
{
  wayland.windowManager.hecate.shell = {
    enable = true;                 # default false
    package = pkgs.hecate-shell;   # from the overlay
    keybinds = true;               # add the end-4 profile as `shell_binds` (keybinds.md)
    layerRules = true;             # blur and per-panel animations for the shell's layers;
                                   # your settings.layer_rules come after and win
    dotfiles = [ "fuzzel" ];       # default; also fish
    extraPackages = [ ];           # more tools on the shell's PATH (hecate itself is always there)
    theming = {
      compositor = true;           # the haiiro frame, motion and blobs (config.md#haiiro-look)
      apps = true;                 # haiiro templates for kitty, foot, GTK and monospace
      qt = true;                   # Qt apps: KDE platform theme + Darkly, the Haiiro colour scheme
      folderIcons = true;          # Papirus-Dark as the KDE icon theme
    };
  };
}
```

### Fixed haiiro templates

Nothing is coloured from the wallpaper. Programs the shell does not draw get fixed haiiro
templates from `nix/haiiro/` (Rosé Pine colours, IBM VGA 8x16 at 16px with Unifont behind it,
square corners):

- **kitty, foot** (`theming.apps`): `~/.config/kitty/haiiro.conf` and
  `~/.config/foot/haiiro.ini`, included from their main config. With Home Manager's
  `programs.kitty`/`programs.foot` on, the include is added to what they write (first, so your
  settings win); otherwise a `kitty.conf`/`foot.ini` with just the include (foot's also
  includes `/etc/xdg/foot/foot.ini`) is written once if there is none. An existing file of
  yours is left alone: add `include haiiro.conf` (kitty) or
  `include=~/.config/foot/haiiro.ini` (foot) to it.
- **GTK** (`theming.apps`): `~/.config/gtk-{3,4}.0/gtk.css` set libadwaita's named colours
  (adw-gtk3-dark reads the same for GTK 3) and square corners. The GTK theme, dark style and
  the haiiro fonts go into dconf once, while those keys are unset. Don't enable Home Manager's
  `gtk` module with it: both write `gtk.css`.
- **monospace** (`theming.apps`): `~/.config/fontconfig/conf.d/50-haiiro-mono.conf` puts IBM
  VGA 8x16, Unifont and Unifont Upper first.
- **Qt** (`theming.qt`): `QT_QPA_PLATFORMTHEME=kde` (plasma-integration, through Home
  Manager's `qt` module) with the Darkly style and `~/.local/share/color-schemes/Haiiro.colors`.
  `~/.config/kdeglobals` is seeded once (writable) from `nix/haiiro/kdeglobals`, or again if it
  still names the old MaterialYouDark scheme (kept as `kdeglobals.material-bak`). The shell's
  settings change its icons and fonts, never its colours.
- **Icons** (`theming.folderIcons`): Papirus-Dark, installed and named in the seeded
  `kdeglobals`.
- **Zen Browser** (not installed by the module): `nix/haiiro/zen/` has `userChrome.css`,
  `userContent.css`, `prefs.nix` (the prefs both stylesheets rely on) and `motion.cfg.js`.
  Wire them yourself, e.g. with the Zen flake's Home Manager module:

  ```nix
  programs.zen-browser = {
    extraPrefsFiles = [ "${hecate}/nix/haiiro/zen/motion.cfg.js" ];
    profiles.default = {
      userChrome = builtins.readFile "${hecate}/nix/haiiro/zen/userChrome.css";
      userContent = builtins.readFile "${hecate}/nix/haiiro/zen/userContent.css";
      settings = import "${hecate}/nix/haiiro/zen/prefs.nix";
    };
  };
  ```

  `motion.cfg.js` cuts Zen's own animations (workspace, glance, folder and Motion springs)
  without `ui.prefersReducedMotion`, so web pages keep their real setting.

- **Vesktop** (not installed by the module): `nix/haiiro/vesktop/` has `haiiro.theme.css`, a
  Vencord theme, and `shiki.nix`, the Rosé Pine theme URL for the ShikiCodeblocks plugin. Wire
  them with Home Manager's `programs.vesktop`:

  ```nix
  programs.vesktop.vencord = {
    themes.haiiro = builtins.readFile "${hecate}/nix/haiiro/vesktop/haiiro.theme.css";
    settings = {
      enabledThemes = [ "haiiro.css" ];
      plugins.ShikiCodeblocks.theme = import "${hecate}/nix/haiiro/vesktop/shiki.nix";
    };
  };
  ```

  The theme cuts Discord's CSS animations and its JavaScript springs itself, so Discord's own
  Reduce Motion setting can stay off. It names the VGA faces through `local()` (the 2x cut is its
  bold), so the haiiro fonts must be installed.

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
- Runtime `PATH` from `sdata/deps-info.md` (see `nix/shell-package.nix` for the list as it is): cliphist, wl-clipboard,
  grim, slurp, wf-recorder, tesseract, translate-shell, jq, curl, wget, bc,
  brightnessctl, ddcutil, playerctl, cava, pavucontrol-qt, wlsunset,
  hyprpicker, fuzzel, ydotool, libnotify, xdg-user-dirs, geoclue2 demo
  agent, plus `hecate` itself for `hecate msg`.
- Python env from `sdata/uv/requirements.in`, all from nixpkgs, behind a small
  `bin/activate` so the shell's `$ILLOGICAL_IMPULSE_VIRTUAL_ENV` scripts work unchanged.
- Fonts: the shell loads its own copies of the haiiro faces; `haiiro-fonts` goes into
  `home.packages` (and system-wide with the NixOS module's `fonts.enable`) for every other
  program.
- Themes: adw-gtk3 and Breeze; Darkly, plasma-integration and Papirus with `shell.theming`
  (above).

What the HM module does with it:

- `systemd.user.services.hecate-shell`: `ExecStart=hecate-shell`,
  `WantedBy=hecate-session.target`, `PartOf=graphical-session.target` (so it stops when
  hecate exits), restart on failure.
- Adds the shell keybind profile to the generated hecate config (your own
  `settings.binds` still win).
- Adds `include = [ "shell.toml" ]` so the shell's settings UI can write
  compositor tweaks to `~/.config/hecate/shell.toml` without touching the
  Nix-made file.
- Installs the dotfiles above into `~/.config` and the haiiro templates
  (see Fixed haiiro templates).
- Leaves mutable shell state alone: `~/.config/illogical-impulse/`
  (`config.json`, written by the settings UI), `~/.local/state/quickshell`,
  `~/.cache/quickshell`.
- Needs from the NixOS side: PAM entry for the QuickShell lock screen
  (`security.pam.services.quickshell`), `services.geoclue2`, `hardware.i2c` for
  ddcutil, `services.upower`, `networking.networkmanager`, `hardware.bluetooth`,
  ydotool and gnome-keyring. The NixOS module's `programs.hecate.shell.enable` turns
  these on (as defaults you can override).

### Login screen

[hecate-greetd](https://github.com/Kex1016/hecate-greetd) is a greetd greeter that runs this
shell's lock screen inside hecate, with the wallpaper, colours, widgets, monitor layout and
keyboard layout of whoever logs in. Its flake has the NixOS module
(`services.hecate-greetd.enable`); the shell shares your look with it on its own (it runs
`hecate-greetd sync` whenever that's installed) and has its settings under Settings →
hecate-greetd.

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
