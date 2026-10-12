# Changelog

## 1.0.0

The first release: phases 0 to 6 of [docs/PLAN.md](docs/PLAN.md).

### Compositor

- Stacking window management: focus and raise policies, snapping to edges and corners with
  resistance, width and height presets, placement policies, window rules, margins and
  regions, minimize, maximize (both, horizontal, vertical), fullscreen, always on top/bottom,
  omnipresent windows.
- Workspaces per output or global, a fixed number or created as needed, named, with
  back-and-forth.
- Borders with per-corner radii that clip window contents; server-side decorations.
- Animations: windows open, close (from a snapshot) and move; border colour fades;
  workspace switches slide; layer surfaces open and close per layer rule (slide from an
  edge, fade, pop in). Cubic-bézier curves, a global slowdown, per-window opt-out.
- Blur: dual Kawase behind layer surfaces per rule, with `xray` (wallpaper only, cached)
  and `ignore_alpha`; ext-background-effect for windows that ask, clipped to their corners.
- DRM/KMS with libseat and VT switching and multiple outputs, a winit backend for nesting
  and a headless one for tests.

### Protocols

xdg-shell, xdg-decoration, layer-shell, session-lock, foreign-toplevel (wlr and ext),
ext-workspace, wlr-screencopy, ext-image-copy-capture (outputs and toplevels),
wlr-output-management, wlr-output-power-management, wlr-gamma-control, hyprland-focus-grab,
ext-background-effect, tearing-control, content-type, drm-syncobj, fifo, commit-timing,
drm-lease, pointer constraints and relative pointer, idle notify and inhibit, data control,
fractional scale, text-input and input-method, tablet, pointer gestures, virtual keyboard.

### Gaming

xwayland-satellite started on demand, VRR with fullscreen windows, tearing per window rule
or content type, direct scanout, `hecate msg gaming` and `hecate msg frames`.

### Input

libinput settings per device (`*`, `type:touchpad`, `type:pointer`, `type:touch`,
`type:tablet`, or a device name), touch screens, touchpad gestures with three-finger
workspace swipes, drawing tablets, keyboard layouts per window or global.

### Tools

`hecate msg` with niri-shaped JSON, window identification (`pick`, `identify` with labels,
`rules`, `tree`, `keys`, `watch`), runtime config overrides (`hecate msg set/unset`),
`hecate validate`, `hecate check`, shell completions and a JSON Schema for the config.

### Shell and Nix

- The bundled end4-pC shell (illogical-impulse), patched for hecate: compositor backend,
  shortcuts over IPC, keybind cheat sheet from `hecate msg binds`, game mode through
  runtime overrides, a hecate settings page (windows, focus, effects, displays with a
  drag-to-arrange map, mouse).
- Flake with the `hecate` and `hecate-shell` packages, a NixOS module (`programs.hecate`,
  with `shell.enable` for the system services the shell needs) and a Home Manager module
  (`wayland.windowManager.hecate`: config as Nix, validated at build time, the shell's
  keybind and layer-rule profiles, its dotfiles and user service).
- Theming from the wallpaper without Plasma: GTK (adw-gtk3 + matugen), Qt (KDE platform
  theme, Darkly, the colour scheme applied through kdeglobals), hecate's border colours, and
  Papirus with folders in the wallpaper's colour.
- Tests: unit and headless integration tests with real Wayland clients, pixel tests on
  Mesa llvmpipe, and NixOS VM tests that boot hecate with a QuickShell bar and with the
  full shell from Home Manager.

### Known limitations

- Outputs that only a secondary GPU can drive are skipped.
- Tearing is tracked and reported, but this Smithay version can't do async page flips, so
  frames still wait for vblank.
