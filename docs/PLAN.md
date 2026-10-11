# hecate: plan

Status: **draft, planning only**. No code yet. This document is the source of
truth and gets edited in place as we iterate on the PR.

This repo is a **monorepo**: the hecate compositor plus a bundled desktop
shell (illogical-impulse + end4-pC, patched for hecate) and the Nix flake
that installs both.

Companion docs:

- [decisions.md](decisions.md): decisions made in review, with reasons
- [labwc-survey.md](labwc-survey.md): what labwc does, what we keep and drop
- [protocols.md](protocols.md): protocol matrix (QuickShell / end4-pC, gaming, desktop)
- [config.md](config.md): config format, schema, examples
- [shell.md](shell.md): the bundled shell (end4-pC + illogical-impulse) and its port
- [keybinds.md](keybinds.md): default binds (labwc + your niri setup + end-4's shell binds)
- [ipc.md](ipc.md): control socket and CLI
- [nix.md](nix.md): flake layout, NixOS and Home Manager modules
- [names.md](names.md): the name (hecate) and where it applies

---

## 1. Goals

hecate is a **stacking** Wayland compositor in the spirit of labwc: small,
predictable, config-file driven, and dependent on external clients for the
shell (bar, launcher, notifications, wallpaper, lock screen).

Hard requirements, in priority order:

1. **QuickShell works fully**, and the **bundled end4-pC shell** runs on
   hecate out of the box. Every Wayland protocol QuickShell's
   compositor-agnostic modules use is implemented (layer-shell, foreign
   toplevel, ext-workspace, screencopy, session lock, idle, shortcuts
   inhibit, focus grab, background effect including blur). The IPC socket
   covers what protocols can't (focused output, window-to-workspace
   mapping, keyboard layout, binds list). See [protocols.md](protocols.md)
   and [shell.md](shell.md). Noctalia works through the same protocols but
   isn't bundled.
2. **Gaming works**: native Wayland/Vulkan, X11 via xwayland-satellite, Wine
   and Proton, Steam, gamescope nested. Pointer lock, relative motion, VRR,
   tearing, direct scanout, explicit sync.
3. **Installs on NixOS via a flake**, configurable entirely from Nix:
   package, NixOS module (session entry, portals, polkit), Home Manager module
   with `settings` written as a Nix attrset (same style as niri-flake's
   `programs.niri.settings`), and the shell with all its dependencies via
   one `shell.enable` switch.
4. **Borders, no decorations.** Configurable border width, colour and
   **per-corner radii**, with content clipped to the rounded shape (niri's
   `geometry-corner-radius` + `clip-to-geometry`, which you use today). No
   titlebars, buttons, menus or shadows from the compositor.

Non-goals (for now): Openbox compatibility (rc.xml, menu.xml, themerc),
server-side titlebars, root/client menus, built-in window switcher OSD,
animations, tiling or scrolling layouts, built-in bar, plugin system.

## 2. Language and core library

**Rust + [Smithay](https://github.com/Smithay/smithay) (0.7.x).**

Why Smithay over wlroots bindings:

- wlroots Rust bindings (`wlroots-rs`) are abandoned; binding wlroots 0.20
  ourselves is a large unsafe FFI surface that tracks an unstable C API.
- Smithay is pure Rust, actively maintained, and already powers production
  compositors (niri, cosmic-comp) that cover our exact needs: rounded corners
  via custom GLES shaders, xwayland-satellite, DRM leasing, explicit sync,
  layer-shell, blur (cosmic-comp, niri forks).
- Smithay 0.7 ships handlers for nearly every protocol on our list
  (verified by reading `smithay/src/wayland/`): xdg-shell, xdg-decoration,
  xdg-dialog, wlr-layer-shell, session-lock, idle-notify/inhibit,
  keyboard-shortcuts-inhibit, pointer-constraints, relative-pointer,
  pointer-gestures, pointer-warp, drm-syncobj, drm-lease, fifo,
  commit-timing, content-type, cursor-shape, fractional-scale, viewporter,
  presentation, xdg-activation, xdg-foreign, ext-foreign-toplevel-list,
  ext-image-copy-capture, background-effect, wlr/ext data-control,
  text-input, input-method, virtual keyboard/pointer, tablet,
  security-context, alpha-modifier.

What we implement ourselves on top of `wayland-protocols(-wlr)` crates (all
have prior art in niri/cosmic-comp we can study):

| protocol                          | needed for                                |
| --------------------------------- | ----------------------------------------- |
| wlr-foreign-toplevel-management   | QuickShell `ToplevelManager`, end4-pC dock/taskbar |
| ext-workspace-v1                  | QuickShell/end4-pC workspaces, waybar     |
| wlr-screencopy-v1                 | QuickShell fallback, grim, hyprpicker, xdpw |
| wlr-output-management-v1          | wlr-randr, kanshi, shell display settings |
| wlr-output-power-management-v1    | wlopm, idle display off                   |
| wlr-gamma-control-v1              | end4-pC night light (via wlsunset)        |
| wp-tearing-control-v1             | games (async page flip)                   |
| hyprland-focus-grab-v1            | end4-pC sidebars/menus close on outside click |
| color-management-v1 (later)       | HDR                                       |

Other crates (indicative): `calloop` (event loop, comes with Smithay),
`xkbcommon`, `input` (libinput), `serde` + `toml` + `serde_json` (config),
`schemars` (JSON Schema for config, editor completion), `tracing`, `clap`,
`notify` (config hot reload), `anyhow`/`thiserror`.

Renderer: Smithay `GlesRenderer` (with `MultiRenderer` for multi-GPU).
Custom GLES shaders for the rounded border, rounded content clipping and blur.

## 3. Architecture

Module layout (one crate to start, split later if useful):

```
src/
  main.rs            CLI (compositor, `msg`, `validate`), logging, backend selection
  state.rs           global State struct, calloop wiring
  backend/
    udev.rs          DRM/KMS + libinput + libseat (real session)
    winit.rs         nested in another compositor (dev)
    headless.rs      tests, virtual outputs
  output.rs          output layout, modes, VRR, scale, transform
  render/
    mod.rs           frame assembly, damage, direct scanout path
    border.rs        rounded border shader element
    clip.rs          rounded-corner clipping of client surfaces
    blur.rs          dual-kawase blur for ext-background-effect
  shell/
    xdg.rs           xdg-shell toplevels + popups
    layer.rs         wlr-layer-shell, exclusive zones, keyboard modes
    xwayland.rs      xwayland-satellite supervisor (on-demand spawn, DISPLAY)
    lock.rs          ext-session-lock
  wm/
    view.rs          View = xdg toplevel (X11 windows arrive as xdg toplevels via satellite)
    stack.rs         per-workspace stacking order, always-on-top/bottom
    workspace.rs     global or per-output workspaces + ext-workspace
    focus.rs         focus policy, activation, focus stealing rules
    placement.rs     center / cursor / cascade / automatic
    interactive.rs   move / resize grabs, snapping, edge resistance
    rules.rs         window rules
  input/
    keyboard.rs      xkb, keybinds, repeat, layouts
    pointer.rs       cursor, mousebinds, constraints, relative motion
    touch.rs, tablet.rs, gestures.rs
    libinput.rs      per-device config
  protocols/         the self-implemented protocols from section 2
  config/
    mod.rs           typed Config struct (serde), defaults, validation
    load.rs          TOML / JSON front-ends, includes
    reload.rs        file watch + atomic apply
  action.rs          the action enum shared by keybinds, mousebinds, IPC
  ipc/               control socket server + `msg` client
```

Scene / stacking order, bottom to top (mirrors labwc's trees):

1. layer-shell `background`
2. layer-shell `bottom`
3. workspace windows: always-on-bottom, normal, always-on-top
4. layer-shell `top`
5. fullscreen window of the output (above `top`, below `overlay`)
6. xdg popups (including X11 menus/tooltips, which satellite maps to popups)
7. layer-shell `overlay`
8. session-lock surfaces (replace everything while locked)
9. cursor, drag icon

Rendering per output builds a list of render elements in that order, with
Smithay's `OutputDamageTracker`. A single fullscreen opaque client with no
overlapping elements takes the **direct scanout** path (`DrmCompositor`
primary-plane assignment), which is what games want.

## 4. Borders (the only decoration)

Requirements: set a border, its width, and the radius of each corner
independently.

Design:

- One custom render element per view, drawn with a fragment shader that
  computes a rounded-rectangle SDF with **four radii** (`top_left`,
  `top_right`, `bottom_right`, `bottom_left`). Antialiased edges.
- Border sits **outside** the client geometry (like labwc's SSD border) so
  the client size is not reduced. Window geometry reported to clients
  excludes the border; placement and snapping include it.
- **Content clipping** (`clip_content`, default on): with rounded corners,
  the client's square corners would poke through. Client surfaces are
  rendered through a clip shader using the inner radii
  (`max(outer_radius - width, 0)` per corner). Clipping disables direct
  scanout for that surface, so it is skipped for fullscreen windows (no
  border, square corners) and can be turned off per rule.
- States: `active`, `inactive`, `urgent` colours. Gradients are a stretch
  goal (the shader makes them cheap to add later).
- Per-window override via window rules (e.g. no border on games, different
  radii for terminals).
- Border is hidden for fullscreen; for maximized it is configurable
  (`keep_on_maximize`, labwc calls this `keepBorder`).
- `width = 0` disables the border but still allows corner clipping (this is
  your current niri look: radius 15, clipped, no visible border).

Decoration negotiation: we advertise **xdg-decoration** and KDE
server-decoration and answer **server-side** by default (your niri
`prefer-no-csd = true`), so GTK/Qt/SDL/Firefox drop their own titlebars. We
then draw nothing but the border. A config option and a window rule allow
`client` mode for apps whose CSD people want to keep. libadwaita apps ignore
xdg-decoration and keep CSD regardless; that's a known limitation we
document, not a bug to fix.

## 5. Window management feature set (v1)

Kept from labwc (adapted, no Openbox semantics):

- Stacking: raise/lower, always-on-top/bottom, minimize, maximize
  (both/horizontal/vertical), fullscreen, omnipresent (sticky).
- Interactive move/resize with mouse binds (`Mod+drag`), resize from border
  edges and corners, edge and window resistance, snap to edges and
  user-defined regions, gaps.
- **Size presets** borrowed from your niri setup: cycle a window through
  `1/3, 1/2, 2/3` of the usable output width (and heights), centered. The
  stacking equivalent of `switch-preset-column-width`.
- Directional focus (`focus_direction left|right|up|down`): focus the
  nearest visible window in that direction on the current output. Stops at
  the output edge; crossing outputs has its own binds (`Mod+Shift+dir`),
  as in your niri config.
- **Workspaces**, two independent settings:
  - `scope`: `global` (labwc style, one set shared by all outputs;
    switching changes every output) or `per_output` (niri/Hyprland style,
    each output has its own set and active workspace).
  - `kind`: `fixed` (`count` workspaces, default 9, max **32**, optional
    `names`) or `auto` (niri style: always exactly one empty workspace at
    the end; empty ones vanish when you leave them; `min` kept even when
    empty, capped at the same max of 32).
  The cap of 32 keeps bars, ext-workspace clients and `Mod+N` bind schemes
  sane; it's a constant we can raise if anyone hits it. Plus
  `auto_back_and_forth` (you use it in niri). Exposed via ext-workspace: one
  group spanning all outputs in `global` scope, one group per output in
  `per_output` scope; auto mode creates and removes ext-workspace handles
  live.
- Focus: click-to-focus or follow-mouse, raise-on-focus, focus-stealing
  rules, xdg-activation token checks.
- Placement policies: center, cursor, cascade, automatic.
- Window rules: match on app_id / title / sandbox engine / toplevel tag
  (X11 windows expose class as app_id through satellite); set border,
  floating geometry, workspace, output, always-on-top, omnipresent,
  skip-taskbar, ignore focus requests, tearing, fixed position, open
  maximized/fullscreen.
- Keybinds and mousebinds mapped to actions ([keybinds.md](keybinds.md),
  action list in [labwc-survey.md](labwc-survey.md#actions)).
- Output margins (labwc `<margin>`) on top of layer-shell exclusive zones.
- **Layer rules**: match layer-shell surfaces by namespace (regex) and set
  `blur`, `blur_popups`, `ignore_alpha`, `xray`, `no_keyboard_focus`,
  `opacity`. The shell profile adds end-4's set.
- Libinput per-device config, xkb layouts, repeat, numlock, touch, tablet.
- Autostart commands and environment variables, from config.
- Hot reload on file change, `hecate msg reload` and SIGHUP.

Dropped: menus, window switcher OSD (the shell can draw one), titlebars and
buttons, shading, themerc, magnifier (maybe later), labnag (replace with a
plain `spawn` of any notification/dialog tool).

## 6. Shell support: QuickShell, bundled end4-pC, Noctalia

### QuickShell

Compositor-agnostic QuickShell modules and the protocol behind each one,
checked against QuickShell's `src/wayland/*/CMakeLists.txt`:

| QuickShell feature                     | protocol                                     |
| -------------------------------------- | -------------------------------------------- |
| `PanelWindow` (bars, docks, overlays)  | wlr-layer-shell-unstable-v1 (v4+)            |
| `PopupWindow`, menus                   | xdg-shell popups + positioner                |
| `ToplevelManager`                      | wlr-foreign-toplevel-management-unstable-v1  |
| Workspaces (`windowmanager` module)    | ext-workspace-v1                             |
| `ScreencopyView`                       | ext-image-copy-capture-v1 + ext-image-capture-source-v1 + ext-foreign-toplevel-list-v1, fallback wlr-screencopy-unstable-v1 |
| `WlSessionLock`                        | ext-session-lock-v1                          |
| `IdleMonitor`                          | ext-idle-notify-v1                           |
| `IdleInhibitor`                        | idle-inhibit-unstable-v1                     |
| `ShortcutInhibitor`                    | keyboard-shortcuts-inhibit-unstable-v1       |
| `BackgroundEffect` (blur behind)       | ext-background-effect-v1                     |
| dmabuf screencopy buffers              | linux-dmabuf-v1                              |
| `HyprlandFocusGrab`                    | hyprland-focus-grab-v1                       |

### Bundled shell: end4-pC

The shell you'll run is end4-pC (a fork of end-4's illogical-impulse
QuickShell config), bundled in this repo with hecate patches applied. It
is Hyprland-heavy (`GlobalShortcut`, `Hyprland.dispatch`, `hyprctl`), but
already has a compositor abstraction (`services/WM.qml` with Hyprland and
niri backends). The port adds a hecate backend next to the niri one, using
our IPC, which deliberately returns niri-shaped JSON. Full analysis and
patch list in [shell.md](shell.md).

### Noctalia

Not bundled, but should work. Noctalia 5.x is a native C++ shell; on an
unknown compositor it uses only protocols, all of which we implement. It
loses focused output, window-to-workspace mapping, keyboard layout and
session exit until someone writes a hecate backend for it against our IPC.

### Things no shell gets from a single protocol

- **Layer-shell correctness** is the big one: exclusive zones per edge,
  `keyboard_interactivity` none/exclusive/on_demand, margin, layer changes
  after map, popups parented to layer surfaces, output hotplug re-sending
  `configure`, per-output surfaces created on output add.
- **Blur (v1)**: a dual-kawase pass over what is behind a surface, with
  config for passes/radius/noise and a global off switch. Two ways to get
  it:
  - the client asks via `ext-background-effect` (QuickShell
    `BackgroundEffect`), restricted to the region it sets;
  - **layer rules** in hecate's config, matched on layer-shell namespace.
    end-4 relies on these: its Hyprland config blurs `quickshell:.*`
    surfaces with `ignore_alpha = 0.79` (only pixels above that alpha get
    blur behind them), `blur_popups`, and `xray = true` globally.
  `xray` mode blurs only the background/bottom layers (the wallpaper), not
  windows behind the panel. That's what end-4 uses, and it's cheap: one
  blurred wallpaper texture per output, re-rendered only when the wallpaper
  changes. Non-xray blur re-samples the live scene each frame.
  This is the only "bling" we add.
- **Global shortcuts**: QuickShell's `GlobalShortcut` is Hyprland-only. Our
  answer is compositor keybinds that `spawn` `qs ipc call <target> <fn>`,
  with separate press and release actions and modifier-only taps (end-4's
  "tap Super for search, hold Super for workspace numbers").
- `XDG_CURRENT_DESKTOP=hecate` and systemd/dbus activation environment
  exported at startup so tray, portals and notifications work.

## 7. Gaming support

| need                                      | how                                        |
| ----------------------------------------- | ------------------------------------------ |
| FPS mouse look                            | pointer-constraints-v1 (lock + confine), relative-pointer-v1 |
| X11 games / Proton default                | xwayland-satellite (section 8). Satellite proxies pointer-constraints, relative-pointer, linux-dmabuf, viewporter, fractional-scale and xdg-activation from Xwayland to us, so implementing those covers X11 games |
| Wine Wayland driver (`winewayland.drv`)   | xdg-shell + pointer protocols + fractional scale; test with `DISPLAY=` unset |
| Low latency / tearing                     | wp-tearing-control-v1 + DRM async page flips, opt-in per window rule and config (`allow_tearing = "fullscreen"` like labwc) |
| VRR / adaptive sync                       | DRM `VRR_ENABLED`, per-output `adaptive_sync = "off" / "on" / "fullscreen"` |
| No compositing overhead                   | direct scanout for fullscreen opaque surfaces |
| NVIDIA and modern Mesa frame pacing       | linux-drm-syncobj-v1 (explicit sync), fifo-v1, commit-timing-v1 |
| Proton HDR (later)                        | color-management-v1 + HDR output metadata; not in Smithay 0.7, stretch goal |
| VR headsets                               | drm-lease-v1                               |
| Steam overlay, Big Picture, gamescope     | gamescope nests fine on any xdg-shell compositor; test matrix item (you have `programs.gamescope` + gamemode enabled) |
| Games that grab keyboard (Alt+Tab, Super) | keyboard-shortcuts-inhibit honoured, with a config-defined escape bind |
| Don't sleep during play                   | idle-inhibit-v1 (+ fullscreen counts as inhibit, configurable) |
| Content-type hints                        | content-type-v1 (`game` → allow tearing/VRR policy) |
| Mixed DPI                                 | fractional-scale-v1 + viewporter; satellite handles X11 scaling |
| Controllers                               | nothing compositor-side (evdev/SDL direct) |

Test matrix (manual at first, see section 11): Steam (via satellite), a
Proton DX11 and a DX12 title, Wine with the Wayland driver, a native Vulkan
game, an SDL3 Wayland game, `gamescope -f -- %command%`, `vkcube`,
Minecraft (Java/GLFW), with gamemode. Your outputs are a good stress case
(200 Hz ultrawide + rotated 1080p + 60 Hz). Check: pointer lock, alt-tab out
of fullscreen, VRR active (`drm_info`), tearing active, frame pacing, no
input lag regression.

## 8. XWayland: xwayland-satellite

Decision: use [xwayland-satellite](https://github.com/Supreeeme/xwayland-satellite)
like niri, no built-in XWM.

- The compositor owns the X11 sockets (`/tmp/.X11-unix/X<n>`, abstract
  socket) and spawns satellite **on the first X11 connection**, handing it
  the listening fds (niri's integration since 25.08 works this way). So
  there's no `spawn-at-startup` and no hardcoded `DISPLAY=:0` like in your
  niri config.
- `DISPLAY` is exported to children and to the systemd/dbus activation
  environment.
- Satellite restarts on crash; X11 clients reconnect.
- Config: `xwayland.enable`, `xwayland.path` (default `xwayland-satellite`
  from `PATH`; the Nix package wraps it in).
- X11 windows appear to us as ordinary xdg toplevels (app_id = WM_CLASS),
  menus/tooltips as popups. Window rules work the same for both.
- Cost: we lose X11 stacking hints and exact override-redirect placement,
  which only matters for a few old apps. If it turns out to break games,
  Smithay's XWM is the fallback; the `View` abstraction keeps that door open.

## 9. Monorepo and NixOS (flake)

Layout:

```
Cargo.toml                 workspace
crates/
  hecate/                  the compositor (+ `hecate msg`, `hecate validate`)
  hecate-ipc/              IPC types, shared with tests and future clients
shell/
  end4-pC/                 git subtree, hecate patches as commits on top
  dots-hyprland/           git subtree, pruned to what we package
  UPSTREAM.md              pinned commits, update procedure, patch list
nix/
  package.nix              hecate
  shell.nix                hecate-shell (end4-pC + QuickShell wrapper + deps)
  python.nix               shell Python env (packages nixpkgs lacks)
  nixos-module.nix
  hm-module.nix
  tests/                   VM tests
flake.nix
docs/
LICENSE                    GPL-3.0-or-later (shell/ keeps its own GPL-3.0 files)
```

Full Nix detail in [nix.md](nix.md). Summary:

- `flake.nix` outputs: `packages.<system>.default` (built with `crane`),
  `overlays.default`, `nixosModules.default`, `homeManagerModules.default`,
  `devShells.default`, `checks` (build, clippy, tests, config schema check,
  `nixos` VM test that starts the compositor and connects a client).
- NixOS module `programs.hecate`: installs the package (wrapped with
  `xwayland-satellite` on `PATH`), registers the session, enables
  `hardware.graphics`, polkit, dconf, xdg portals with a
  `hecate-portals.conf`.
- Home Manager module `wayland.windowManager.hecate`: `settings` is a plain
  Nix attrset serialised to TOML. Pure Nix, no strings of another language.
  Build-time validation with `hecate validate`. `systemd.enable` creates
  `hecate-session.target` bound to `graphical-session.target`.
- Portals: your niri config uses `xdg-desktop-portal-gnome` for ScreenCast.
  That relies on Mutter's D-Bus API, which niri emulates and we won't.
  hecate uses `xdg-desktop-portal-wlr` for ScreenCast/Screenshot (it
  supports ext-image-copy-capture, including window capture) and keeps gtk
  for FileChooser and gnome-keyring for Secret.
- `packages.hecate-shell` + `wayland.windowManager.hecate.shell.enable`:
  the bundled shell with its Qt modules, CLI tools, fonts, Python env and
  dotfiles, started as a user service. See [shell.md](shell.md).
- Supported systems: `x86_64-linux`, `aarch64-linux`.

## 10. Configuration

Full detail in [config.md](config.md). Decision:

- **TOML** (`~/.config/hecate/config.toml`) as the primary format, **JSON**
  accepted with the same schema. Pure data, no embedded interpreter.
- Why not Lua (the earlier proposal): you configure everything from Nix, and
  Nix already gives you variables, loops and per-host conditionals. A second
  programming language in the config adds an interpreter, a sandbox and
  reload edge cases while doing what Nix already does. Data formats map 1:1
  to Nix attrsets: `pkgs.formats.toml` writes them, and `builtins.fromTOML`
  can even read a hand-written file back into Nix.
- Why TOML over JSON: comments and nicer hand-editing for non-Nix users.
  JSON costs nothing extra (same serde struct) and suits generated configs.
- Binds are a map from key combo to action, the same shape as niri-flake's
  `binds."Mod+Q".action`, so your Nix `binds` helper ports nearly as-is.
- `include = [ ... ]` for splitting files; a JSON Schema generated from the
  Rust types gives editor completion (taplo) and drives the Nix module docs.
- Errors: invalid config on reload keeps the old config, logs the error and
  runs `error_command` (e.g. `notify-send`).

## 11. Development and testing

- `cargo run -- --backend winit` runs nested inside any compositor for
  development (Smithay winit backend). `Mod` maps to `Alt` when nested,
  like niri.
- Headless backend for integration tests: spawn compositor, connect a
  test client built with `wayland-client`, assert protocol behaviour
  (layer-shell exclusive zones, workspace events, foreign toplevel state,
  IPC responses).
- NixOS VM test in `checks` (QEMU, virtio-gpu) boots to the session and
  runs `wayland-info` + a QuickShell smoke config.
- CI: GitHub Actions running `nix flake check` (fmt, clippy `-D warnings`,
  tests, VM test).
- Logging via `tracing`; `HECATE_LOG=debug`.
- License: **GPL-3.0-or-later**. Smithay is MIT (compatible). labwc is
  GPL-2.0-only; we study it but don't copy code.

## 12. Phases

Each phase ends usable and merged. **v1.0 = phases 0 to 6.**

0. **Skeleton.** Cargo project, flake with package + devShell, CI, winit
   backend, one xdg-shell window rendered. `nix run` works.
1. **Real session.** udev/DRM/libinput/libseat backend, multi-output,
   hotplug, keyboard/pointer, xdg-shell toplevels + popups, basic stacking
   and focus, TOML/JSON config with keybinds, spawn, exit, reload.
   NixOS module + session entry so it can be picked in a display manager.
2. **Borders.** Rounded per-corner border shader, content clipping,
   active/inactive colours, xdg-decoration server-side negotiation.
3. **Shell baseline.** Layer-shell (full), wlr-foreign-toplevel,
   ext-foreign-toplevel-list, workspaces (both scopes, fixed + auto) + ext-workspace,
   session-lock, idle notify/inhibit, shortcuts inhibit, screencopy (ext +
   wlr), data-control, xdg-activation, output-management, output-power,
   gamma, focus-grab. **IPC socket + `hecate msg`** (niri-shaped JSON).
   Press/release and modifier-tap binds. Home Manager module with systemd
   target. Portals config. Test with a plain QuickShell config.
4. **Gaming.** xwayland-satellite integration, pointer constraints/relative
   pointer, direct scanout, VRR, tearing control, drm-syncobj,
   fifo/commit-timing, drm-lease, content-type. Run the test matrix.
5. **Bundled shell.** Subtree end4-pC + pruned dots-hyprland, hecate
   backend and patches ([shell.md](shell.md)), `hecate-shell` package,
   Python env, Home Manager `shell.enable`, shell keybind profile. VM test
   boots into the full desktop.
6. **WM polish + blur.** Interactive move/resize, snapping, regions,
   resistance, size presets, placement policies, window rules, margins,
   minimize/maximize/fullscreen edge cases, libinput per-device config,
   touch, tablet, gestures, IME. **Blur**: layer rules (xray first, which
   end-4 needs) and ext-background-effect. Default keybinds finalised.
   v1.0 release.
7. **After v1.** Noctalia backend, HDR / color management, border
   gradients, zoom, typed Nix options generated from the JSON Schema.

## 13. Open questions

1. **Bind conflicts** between your niri binds and end-4's shell binds
   (see [keybinds.md](keybinds.md#conflicts)). Proposal: end-4's shell binds
   win, since that's the desktop you picked, and your niri binds that clash
   move or go.
2. **Pruning dots-hyprland**: OK to vendor only the parts we package (keeps
   the repo small), or do you want the whole tree in `shell/` for reference?
3. **Repo rename**: rename `kex1016/wl-hex` to `hecate` on GitHub? (Your
   call, I can't do it from here.)
4. **Layer animations**: end-4's Hyprland config also slides panels in
   (`animation = "slide"` layer rules). Most of end-4's motion is inside
   QML and keeps working; only the compositor-side open/close slide of
   layer surfaces would be missing. labwc's philosophy says no animations.
   Keep it that way, or allow a small opt-in layer open/close slide?
