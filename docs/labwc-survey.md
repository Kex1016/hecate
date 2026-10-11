# labwc survey

Notes from reading labwc `master` (0.20.2, wlroots 0.20) and
labwc.github.io. Purpose: know what we're replacing and what to skip.

## Shape of the codebase

~38k lines of C, meson build, wlroots scene graph. Biggest files:

| file                    | lines | what                                         |
| ----------------------- | ----- | -------------------------------------------- |
| `src/view.c`            | 2581  | the View abstraction, all window state       |
| `src/config/rcxml.c`    | 2182  | rc.xml parser (element/attribute agnostic)   |
| `src/theme.c`           | 1881  | Openbox themerc parsing and rendering        |
| `src/menu/menu.c`       | 1718  | root/client menus, pipemenus                 |
| `src/action.c`          | 1686  | action dispatch                              |
| `src/input/cursor.c`    | 1676  | pointer, mousebinds, constraints             |
| `src/output.c`          | 1384  | outputs, output-management                   |
| `src/xwayland.c`        | 1331  | XWM views                                    |
| `src/xdg.c`             | 1185  | xdg-shell views                              |
| `src/seat.c`            |  995  | seat, input device setup                     |
| `src/server.c`          |  919  | global setup, every protocol global created  |

Of that, roughly **9k lines** we would not reimplement at all: `theme.c`,
`menu/`, `ssd/` (titlebar, buttons, shadow), `cycle/` (switcher OSD),
`img/` (xbm/xpm/png/svg icon loading), `scaled-buffer/` (font/icon buffers),
`desktop-entry.c`, `magnifier.c`. The XML parser goes too.

## Concepts worth copying

- **`struct view`** unifies xdg toplevels and X11 windows behind one
  interface (`view_impl` vtable: configure, close, set_activated, maximize,
  fullscreen, minimize...). We do the same with a Rust enum/trait.
- **Scene trees** in fixed z-order created in `server.c`: per-layer trees,
  `workspace_tree` (one subtree per workspace), `xdg_popup_tree`,
  `unmanaged_tree` (X11 override-redirect), `menu_tree`, plus session-lock.
  Our ordering in PLAN.md section 3 mirrors this.
- **Pending vs current geometry** on views (`view->pending`,
  `view->current`) to handle async configure/ack. Same approach needed.
- **Output usable area** = output minus layer-shell exclusive zones minus
  user `<margin>`. Used by placement, maximize, snapping.
- **Edges/resistance/snap** (`edges.c`, `resistance.c`, `snap.c`) as
  separate small modules.
- **Window rules** evaluated at map time and on property change,
  `match.c` does glob matching on identifier/title.
- **Tearing**: `<allowTearing>` yes/no/fullscreen/fullscreenForced plus a
  per-window `ToggleTearing` action. Copy the semantics.
- **Adaptive sync**: `<adaptiveSync>` yes/no/fullscreen. Copy.
- **Keybind flags**: `layoutDependent`, `onRelease`, `allowWhenLocked`.
  Copy all three.
- **Reconfigure** keeps state, reloads config, re-applies rules and borders.
- `xwaylandPersistence`: keep XWayland running once started (lazy start
  by default). Copy.
- Session/environment handling (`config/session.c`): reads
  `~/.config/labwc/environment`, runs `autostart`/`shutdown`, updates the
  dbus/systemd activation environment. We need the same, minus shell
  scripts if we fold autostart into config.

## Borders in labwc

`src/ssd/ssd-border.c` builds borders from four `wlr_scene_rect`s per
active/inactive state; rounded corners only exist on the titlebar
(`<theme><cornerRadius>`, single value, top corners only, drawn as cairo
buffers). No per-corner radii, no content clipping. Our shader-based border
is a different design, not a port.

## Protocol globals labwc creates

From `grep` over `src/`: compositor, subcompositor, xdg-shell,
layer-shell, xdg-decoration, KDE server-decoration, data-device,
primary-selection, wlr/ext data-control, linux-dmabuf, drm-syncobj,
drm (wl_drm), presentation, viewporter, single-pixel-buffer,
fractional-scale, alpha-modifier, color-management, color-representation,
xdg-output, output-management, output-power, gamma-control, screencopy,
export-dmabuf, ext-image-copy-capture (+ output and toplevel sources),
wlr-foreign-toplevel, ext-foreign-toplevel-list, ext-workspace,
session-lock, idle-notify, idle-inhibit, xdg-activation, xdg-foreign
v1/v2, xdg-toplevel-icon, xdg-dialog, pointer-constraints,
relative-pointer, pointer-gestures, cursor-shape, virtual
keyboard/pointer, text-input-v3, input-method-v2, tablet-v2,
tearing-control, drm-lease, security-context, wl_fixes, xwayland.

That's effectively our checklist too; see [protocols.md](protocols.md).

## Actions

labwc's action list (from `docs/labwc-actions.5.scd`), with our verdict:

| keep                                                   | drop / replace                         |
| ------------------------------------------------------ | -------------------------------------- |
| Close, Kill, Execute (`spawn`), Exit, Reconfigure      | ShowMenu (no menus)                    |
| Focus, Unfocus, Raise, Lower, Iconify (`minimize`)     | Shade, Unshade, ToggleShade            |
| Maximize, UnMaximize, ToggleMaximize, ToggleFullscreen | SetDecorations, ToggleDecorations → `ToggleBorder` |
| ToggleAlwaysOnTop, ToggleAlwaysOnBottom, ToggleOmnipresent | NextWindow/PreviousWindow OSD → plain focus-cycle without OSD (QuickShell can draw one) |
| Move, Resize (interactive), MoveTo, ResizeTo, MoveRelative, ResizeRelative, MoveToCursor, AutoPlace | ToggleMagnify, ZoomIn, ZoomOut (later, maybe) |
| MoveToEdge, GrowToEdge, ShrinkToEdge, FitToOutput      | DebugToggleKeyStateIndicator           |
| SnapToEdge, ToggleSnapToEdge, UnSnap, SnapToRegion, ToggleSnapToRegion | If, ForEach (Lua covers conditionals at config time) |
| GoToDesktop, SendToDesktop (`workspace`, `move_to_workspace`) |                                  |
| FocusOutput, MoveToOutput, WarpCursor, HideCursor      |                                        |
| ToggleKeybinds (games), ToggleTearing                  |                                        |
| VirtualOutputAdd/Remove (headless outputs, useful for streaming) |                              |
| Scroll-wheel/tablet mouse emulation toggles            |                                        |
| ToggleShowDesktop                                      |                                        |

## Config surface (rc.xml) worth keeping, renamed

`core` (gap, adaptiveSync, allowTearing, autoEnableOutputs, reuseOutputMode,
xwaylandPersistence, primarySelection, promptCommand → drop,
errorCommand), `placement` (policy, cascadeOffset), `resistance`, `focus`
(followMouse, followMouseRequiresMovement, raiseOnFocus,
raiseOnFocusDelay), `snapping` (range, cornerRange, overlay, topMaximize,
notifyClient), `regions`, `desktops` (names, initial, popupTime → drop),
`margin`, `resize` (cornerRange, minimumArea; popupShow → drop),
`keyboard` (numlock, layoutScope, keybinds, repeat), `mouse`
(doubleClickTime, mousebinds by context), `touch`, `tablet`, `tabletTool`,
`libinput` (all device options), `windowRules`.

Dropped: `theme` (except border-relevant bits, which move to `border`),
`windowSwitcher`, `menu`, `magnifier`, titlebar layout/fonts.

## Docs site notes (labwc.github.io)

- `integration.md` is the best reference for what a bare compositor needs
  from the ecosystem: layer-shell + foreign-toplevel for panels,
  wlr-randr/kanshi for outputs, grim/slurp for screenshots, swaylock +
  swayidle + wlopm for locking/idle, cliphist for clipboard, fcitx5/ibus,
  portals config (`labwc-portals.conf`, `GDK_DEBUG=no-portals` caveat).
  We should ship an equivalent `docs/integration.md` focused on QuickShell.
- `README.md` gaming section is one line (pointer confinement since 0.6.2,
  otherwise use gamescope). We aim much further; see PLAN.md section 7.
- labwc deliberately has **no IPC** ("cannot be controlled with dbus,
  sway/i3/custom-IPC"). Whether we follow that is open question 5.
