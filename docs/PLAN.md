# wl-hex: plan

Status: **draft, planning only**. No code yet. This document is the source of
truth and gets edited in place as we iterate on the PR.

Companion docs:

- [labwc-survey.md](labwc-survey.md): what labwc does, what we keep and drop
- [protocols.md](protocols.md): protocol matrix (QuickShell, gaming, desktop)
- [config.md](config.md): config format, schema sketch, examples
- [nix.md](nix.md): flake layout, NixOS and Home Manager modules

---

## 1. Goals

wl-hex is a **stacking** Wayland compositor in the spirit of labwc: small,
predictable, config-file driven, and dependent on external clients for the
shell (bar, launcher, notifications, wallpaper, lock screen).

Hard requirements, in priority order:

1. **QuickShell works fully.** Every Wayland protocol QuickShell's
   compositor-agnostic modules use is implemented (layer-shell, foreign
   toplevel, ext-workspace, screencopy, session lock, idle, shortcuts inhibit,
   background effect). See [protocols.md](protocols.md).
2. **Gaming works**: native Wayland/Vulkan, native X11, Wine and Proton
   (XWayland and the Wine Wayland driver), Steam, gamescope nested. Pointer
   lock, relative motion, VRR, tearing, direct scanout, explicit sync.
3. **Installs on NixOS via a flake**: package, NixOS module (session entry,
   portals, polkit), Home Manager module (typed settings to config file).
4. **Borders, no decorations.** Configurable border width, colour and
   **per-corner radii**. No titlebars, buttons, menus or shadows from the
   compositor.

Non-goals (for now): Openbox compatibility (rc.xml, menu.xml, themerc),
server-side titlebars, root/client menus, built-in window switcher OSD,
animations, tiling layouts, built-in bar, plugin system.

## 2. Language and core library

**Rust + [Smithay](https://github.com/Smithay/smithay) (0.7.x).**

Why Smithay over wlroots bindings:

- wlroots Rust bindings (`wlroots-rs`) are abandoned; binding wlroots 0.20
  ourselves is a large unsafe FFI surface that tracks an unstable C API.
- Smithay is pure Rust, actively maintained, and already powers production
  compositors (niri, cosmic-comp) that cover our exact needs: rounded corners
  via custom GLES shaders, XWayland, DRM leasing, explicit sync, layer-shell.
- Smithay 0.7 ships handlers for nearly every protocol on our list
  (verified by reading `smithay/src/wayland/`): xdg-shell, wlr-layer-shell,
  session-lock, idle-notify/inhibit, keyboard-shortcuts-inhibit,
  pointer-constraints, relative-pointer, pointer-gestures, pointer-warp,
  drm-syncobj, drm-lease, fifo, commit-timing, content-type, cursor-shape,
  fractional-scale, viewporter, presentation, xdg-activation, xdg-foreign,
  ext-foreign-toplevel-list, ext-image-copy-capture, background-effect,
  wlr/ext data-control, text-input, input-method, virtual keyboard/pointer,
  tablet, security-context, alpha-modifier, xwayland-shell, XWM.

What we implement ourselves on top of `wayland-protocols(-wlr)` crates (all
have prior art in niri/cosmic-comp we can study):

| protocol                          | needed for                                |
| --------------------------------- | ----------------------------------------- |
| wlr-foreign-toplevel-management   | QuickShell `ToplevelManager`, taskbars    |
| ext-workspace-v1                  | QuickShell workspaces, waybar             |
| wlr-screencopy-v1                 | QuickShell screencopy fallback, grim, portal |
| wlr-output-management-v1          | wlr-randr, kanshi, way-displays           |
| wlr-output-power-management-v1    | wlopm, idle display off                   |
| wlr-gamma-control-v1              | gammastep, wlsunset                       |
| wp-tearing-control-v1             | games (async page flip)                   |
| hyprland-focus-grab-v1 (optional) | QuickShell popup dismissal                |

Other crates (indicative): `calloop` (event loop, comes with Smithay),
`xkbcommon`, `input` (libinput), `mlua` (config, see below), `serde`,
`tracing`, `clap`, `notify` (config hot reload), `anyhow`/`thiserror`.

Renderer: Smithay `GlesRenderer` (with `MultiRenderer` for multi-GPU).
Custom GLES shaders for the rounded border and rounded content clipping.

## 3. Architecture

Module layout (one crate to start, split later if useful):

```
src/
  main.rs            CLI, logging, backend selection
  state.rs           global State struct, calloop wiring
  backend/
    udev.rs          DRM/KMS + libinput + libseat (real session)
    winit.rs         nested in another compositor (dev)
    headless.rs      tests
  output.rs          output layout, modes, VRR, scale, transform
  render/
    mod.rs           frame assembly, damage, direct scanout path
    border.rs        rounded border shader element
    clip.rs          rounded-corner clipping of client surfaces
  shell/
    xdg.rs           xdg-shell toplevels + popups
    layer.rs         wlr-layer-shell, exclusive zones, keyboard modes
    xwayland.rs      XWM integration, override-redirect
    lock.rs          ext-session-lock
  wm/
    view.rs          View = xdg toplevel | X11 window, unified API
    stack.rs         per-workspace stacking order, always-on-top/bottom
    workspace.rs     workspaces + ext-workspace
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
    lua.rs           Lua front-end
    reload.rs        file watch + atomic apply
  action.rs          the action enum shared by keybinds, mousebinds, IPC
  ipc.rs             optional control socket (phase 6)
```

Scene / stacking order, bottom to top (mirrors labwc's trees):

1. layer-shell `background`
2. layer-shell `bottom`
3. workspace windows: always-on-bottom, normal, always-on-top
4. layer-shell `top`
5. fullscreen window of the output (above `top`, below `overlay`)
6. XWayland override-redirect (menus, tooltips)
7. xdg popups
8. layer-shell `overlay`
9. session-lock surfaces (replace everything while locked)
10. cursor, drag icon

Rendering per output builds a list of render elements in that order, with
Smithay's `OutputDamageTracker`. A single fullscreen opaque client with no
overlapping elements takes the **direct scanout** path (`DrmCompositor`
primary-plane assignment), which is what games want.

## 4. Borders (the only decoration)

Requirements from the brief: set a border, its width, and the radius of each
corner independently.

Design:

- One custom render element per view, drawn with a fragment shader that
  computes a rounded-rectangle SDF with **four radii** (`top_left`,
  `top_right`, `bottom_right`, `bottom_left`). Antialiased edges.
- Border sits **outside** the client geometry (like labwc's SSD border) so
  the client size is not reduced. Window geometry reported to clients
  excludes the border; placement and snapping include it.
- **Content clipping**: with rounded corners, the client's square corners
  would poke through. Client surfaces are rendered through a clip shader
  using the inner radii (`max(outer_radius - width, 0)` per corner). Clipping
  disables direct scanout for that surface, so it is skipped for fullscreen
  windows (no border, square corners) and can be turned off per rule.
- States: `active`, `inactive`, `urgent` colours. Gradients are a stretch
  goal (the shader makes them cheap to add later).
- Per-window override via window rules (e.g. no border on a specific app,
  different radii for terminals).
- Border is hidden for fullscreen; for maximized it is configurable
  (`keep_border_on_maximize`, labwc calls this `keepBorder`).
- `width = 0` disables the border but still allows corner clipping.

Decoration negotiation: we advertise **xdg-decoration** and KDE
server-decoration and answer **server-side** by default, so GTK/Qt/SDL/Firefox
drop their own titlebars. We then draw nothing but the border. A config option
and a window rule allow `client` mode for apps whose CSD people want to keep.
libadwaita apps ignore xdg-decoration and keep CSD regardless; that's a known
limitation we document, not a bug to fix.

## 5. Window management feature set (v1)

Kept from labwc (adapted, no Openbox semantics):

- Stacking: raise/lower, always-on-top/bottom, minimize (iconify), maximize
  (both/horizontal/vertical), fullscreen, omnipresent (sticky).
- Interactive move/resize with mouse binds (`super+drag` by default), resize
  from border edges and corners, edge and window resistance, snap to edges
  and user-defined regions, gaps.
- Workspaces: fixed, named, per-compositor (not per-output) to match labwc.
  Exposed via ext-workspace.
- Focus: click-to-focus or follow-mouse, raise-on-focus, focus-stealing
  rules, xdg-activation token checks.
- Placement policies: center, cursor, cascade, automatic.
- Window rules: match on app_id / title / X11 class / sandbox engine;
  set border, floating geometry, workspace, output, always-on-top,
  skip-taskbar, ignore focus requests, tearing, fixed position.
- Keybinds and mousebinds mapped to actions (see `action.rs` list in
  [labwc-survey.md](labwc-survey.md#actions)).
- Output margins (labwc `<margin>`) on top of layer-shell exclusive zones.
- Libinput per-device config, xkb layouts, repeat, numlock, touch, tablet.
- Autostart commands and environment file.
- Hot reload on file change and `wl-hex msg reload` / SIGHUP.

Dropped: menus, window switcher OSD (use QuickShell), titlebars and buttons,
shading, themerc, magnifier (maybe later), labnag (replace with a plain
`spawn` of any notification/dialog tool).

## 6. QuickShell support

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
| `HyprlandFocusGrab` (optional)         | hyprland-focus-grab-v1                       |

Things QuickShell needs that are not a single protocol:

- **Layer-shell correctness** is the big one: exclusive zones per edge,
  `keyboard_interactivity` none/exclusive/on_demand, margin, layer changes
  after map, popups parented to layer surfaces, output hotplug re-sending
  `configure`, per-output surfaces created on output add.
- **Blur** for `BackgroundEffect`: implement a dual-kawase blur pass of what
  is behind the surface, restricted to the region the client sets. This is
  the only "bling" we add and it is opt-in by the client. Phase 5.
- **Global shortcuts**: QuickShell's `GlobalShortcut` is Hyprland-only. Our
  answer is compositor keybinds that `spawn` `qs ipc call <target> <fn>`.
  Documented with examples. No custom protocol.
- **Workspace / window info** beyond protocols (e.g. focused output, layout
  data): covered by the optional IPC in phase 6 if protocols prove
  insufficient. Decide after QuickShell configs are tested against phase 3.
- `XDG_CURRENT_DESKTOP=wl-hex` and systemd/dbus activation environment
  exported at startup so QuickShell's tray, portals and notifications work.

## 7. Gaming support

| need                                      | how                                        |
| ----------------------------------------- | ------------------------------------------ |
| FPS mouse look                            | pointer-constraints-v1 (lock + confine), relative-pointer-v1 |
| X11 games / Proton default                | XWayland via Smithay XWM. Xwayland is itself a client of pointer-constraints and relative-pointer, so those protocols cover X11 games too; xwayland-keyboard-grab for games that grab the keyboard |
| Wine Wayland driver (`winewayland.drv`)   | xdg-shell + pointer protocols + fractional scale; test with `DISPLAY=` unset |
| Low latency / tearing                     | wp-tearing-control-v1 + DRM async page flips, opt-in per window rule and config (`allow_tearing = "fullscreen"` like labwc) |
| VRR / adaptive sync                       | DRM `VRR_ENABLED`, config `adaptive_sync = "off" / "on" / "fullscreen"` |
| No compositing overhead                   | direct scanout for fullscreen opaque surfaces |
| NVIDIA and modern Mesa frame pacing       | linux-drm-syncobj-v1 (explicit sync), fifo-v1, commit-timing-v1 |
| Proton HDR (later)                        | color-management-v1 + HDR output metadata; stretch goal, Smithay support needs checking |
| VR headsets                               | drm-lease-v1                               |
| Steam overlay, Big Picture, gamescope     | gamescope nests fine on any xdg-shell compositor; test matrix item |
| Games that grab keyboard (Alt+Tab, Super) | keyboard-shortcuts-inhibit honoured, with a config-defined escape bind |
| Don't sleep during play                   | idle-inhibit-v1 (+ fullscreen counts as inhibit, configurable) |
| Content-type hints                        | content-type-v1 (`game` → allow tearing/VRR policy) |
| Mixed DPI                                 | fractional-scale-v1 + viewporter; XWayland clients unscaled by default, opt-in `xwayland_scale` setting |
| Controllers                               | nothing compositor-side (evdev/SDL direct) |

Test matrix (manual at first, see section 10): Steam (XWayland), a Proton
DX11 and a DX12 title, Wine with the Wayland driver, a native Vulkan game,
an SDL3 Wayland game, `gamescope -f -- %command%`, `vkcube`, Minecraft
(Java/GLFW). Check: pointer lock, alt-tab out of fullscreen, VRR active
(`drm_info`), tearing active, frame pacing, no input lag regression.

## 8. NixOS (flake)

Full detail in [nix.md](nix.md). Summary:

- `flake.nix` outputs: `packages.<system>.default` (built with `crane` or
  `rustPlatform.buildRustPackage`), `overlays.default`,
  `nixosModules.default`, `homeManagerModules.default`, `devShells.default`,
  `checks` (build, clippy, tests, `nixos` VM test that starts the compositor
  headless and connects a client).
- NixOS module `programs.wl-hex`: installs the package, registers the
  session (`services.displayManager.sessionPackages`), enables
  `hardware.graphics`, polkit, dconf, xdg portals (`xdg-desktop-portal-wlr` for
  screencast + `xdg-desktop-portal-gtk` for file chooser) with a
  `wl-hex-portals.conf`, XWayland.
- Home Manager module `wayland.windowManager.wl-hex`: `settings` attrset →
  config file (via `lib.generators.toLua` or JSON), `extraConfig` raw Lua,
  `systemd.enable` creating `wl-hex-session.target` bound to
  `graphical-session.target` so QuickShell and friends start as user
  services.
- Supported systems: `x86_64-linux`, `aarch64-linux`.
- Binary cache: optional Cachix later.

## 9. Configuration

Full detail in [config.md](config.md). Decision proposed for review:

- **Lua** as the primary format (`~/.config/wl-hex/init.lua`), evaluated once
  per load in a sandboxed `mlua` state, returning a table. No runtime
  callbacks in v1 so behaviour stays declarative and reloadable.
- The table deserialises (via `mlua`'s serde support) into the same typed
  `Config` struct that also accepts **`config.json`**. One schema, two
  front-ends; Nix can emit either.
- Why not XML: Openbox is dropped, so its schema buys nothing.
  Why Lua over pure JSON: comments, variables, loops (workspace binds 1..9),
  per-host conditionals, and Nix has `lib.generators.toLua`. JSON stays
  available for people who want zero logic.
- Errors: invalid config on reload keeps the old config and logs the error
  plus spawns an optional `error_command` (e.g. `notify-send`).

## 10. Development and testing

- `cargo run -- --backend winit` runs nested inside any compositor for
  development (Smithay winit backend).
- Headless backend for integration tests: spawn compositor, connect a
  test client built with `wayland-client`, assert protocol behaviour
  (layer-shell exclusive zones, workspace events, foreign toplevel state).
- NixOS VM test in `checks` (QEMU, virtio-gpu) boots to the session and
  runs `wayland-info` + a QuickShell smoke config.
- CI: GitHub Actions running `nix flake check` (fmt, clippy `-D warnings`,
  tests, VM test).
- Logging via `tracing`; `WL_HEX_LOG=debug`.
- License: GPL-3.0-or-later proposed (Smithay is MIT; labwc is GPL-2.0-only,
  we don't copy its code, only study it). To confirm.

## 11. Phases

Each phase ends usable and merged.

0. **Skeleton.** Cargo project, flake with package + devShell, CI, winit
   backend, one xdg-shell window rendered. `nix run` works.
1. **Real session.** udev/DRM/libinput/libseat backend, multi-output,
   hotplug, keyboard/pointer, xdg-shell toplevels + popups, basic stacking
   and focus, Lua/JSON config with keybinds, spawn, exit, reload.
   NixOS module + session entry so it can be picked in a display manager.
2. **Borders.** Rounded per-corner border shader, content clipping,
   active/inactive colours, xdg-decoration server-side negotiation.
3. **QuickShell baseline.** Layer-shell (full), wlr-foreign-toplevel,
   ext-foreign-toplevel-list, ext-workspace + workspaces, session-lock,
   idle notify/inhibit, shortcuts inhibit, screencopy (ext + wlr),
   data-control, xdg-activation, output-management, output-power, gamma.
   Home Manager module with systemd target. Portals config.
4. **Gaming.** XWayland (XWM), pointer constraints/relative pointer,
   direct scanout, VRR, tearing control, drm-syncobj, fifo/commit-timing,
   drm-lease, content-type. Run the test matrix.
5. **WM polish.** Interactive move/resize, snapping, regions, resistance,
   placement policies, window rules, margins, minimize/maximize/fullscreen
   edge cases, libinput per-device config, touch, tablet, gestures, IME.
   Background-effect blur.
6. **Optional IPC + CLI.** Unix socket, JSON messages: run action, query
   outputs/windows/workspaces, subscribe to events. `wl-hex msg ...`. Only if
   phase 3 testing shows QuickShell configs need more than protocols.
7. **Stretch.** HDR / color management, border gradients, magnifier/zoom,
   screen recording niceties.

## 12. Open questions for review

1. **Name**: binary `wl-hex`, config dir `~/.config/wl-hex/`, env prefix
   `WL_HEX_`? Or shorten to `hex`?
2. **Config**: Lua primary + JSON alternate (proposed), or JSON only, or
   something else (KDL/TOML)?
3. **XWayland**: integrated Smithay XWM (proposed, needed for proper
   stacking-WM semantics and override-redirect placement) vs
   `xwayland-satellite` like niri (less code, rootless-as-Wayland-client,
   but X11 windows lose stacking hints).
4. **Workspaces**: global (labwc style, proposed) or per-output?
5. **IPC**: do you want a control socket at all, or should we stay
   protocol-only like labwc and use `qs ipc` from keybinds?
6. **Blur** for QuickShell `BackgroundEffect`: in scope for v1 or later?
7. **License**: GPL-3.0-or-later OK?
8. **Default keybinds**: start from labwc's defaults (see survey) or your own
   set?
