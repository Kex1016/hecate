# Protocol matrix

Legend for **Source**: `smithay` = handler exists in Smithay 0.7
(`src/wayland/`), `own` = we implement on top of the protocol crates.
**Phase** refers to PLAN.md section 12. **Why** lists who needs it:
QS = QuickShell, including the bundled end4-pC shell (Noctalia 5.x binds the same set), G = gaming, D = general desktop / ecosystem tools.

## Core

| protocol                        | source  | phase | why                        |
| ------------------------------- | ------- | ----- | -------------------------- |
| wl_compositor, wl_subcompositor | smithay | 0     | everything                 |
| wl_shm, linux-dmabuf-v1         | smithay | 0     | buffers; QS dmabuf capture |
| xdg-shell                       | smithay | 0     | all apps, QS popups        |
| wl_seat, wl_data_device         | smithay | 1     | input, clipboard, DnD      |
| primary-selection-v1            | smithay | 1     | middle-click paste         |
| xdg-output-v1                   | smithay | 1     | output names for clients   |
| viewporter, fractional-scale-v1 | smithay | 1     | HiDPI, G (Wine Wayland)    |
| presentation-time               | smithay | 1     | G, video players           |
| single-pixel-buffer-v1          | smithay | 1     | cheap solid surfaces       |
| cursor-shape-v1                 | smithay | 1     | D                          |
| xdg-decoration, KDE server-deco | smithay | 2     | ask clients to drop CSD    |
| wl_fixes                        | smithay | 1     | D                          |

## Shell / QuickShell

| protocol                              | source  | phase | why                    |
| ------------------------------------- | ------- | ----- | ---------------------- |
| wlr-layer-shell-unstable-v1           | smithay | 3     | QS `PanelWindow`, bars |
| wlr-foreign-toplevel-management-v1    | own     | 3     | QS `ToplevelManager`, taskbars |
| ext-foreign-toplevel-list-v1          | smithay | 3     | QS screencopy of toplevels |
| ext-workspace-v1                      | own     | 3     | QS workspaces, waybar  |
| ext-image-capture-source-v1           | smithay | 3     | QS `ScreencopyView`, portals |
| ext-image-copy-capture-v1             | smithay | 3     | QS `ScreencopyView`, grim |
| wlr-screencopy-unstable-v1            | own     | 3     | QS fallback, older tools, xdpw |
| ext-session-lock-v1                   | smithay | 3     | QS `WlSessionLock`, swaylock |
| ext-idle-notify-v1                    | smithay | 3     | QS `IdleMonitor`, swayidle |
| idle-inhibit-unstable-v1              | smithay | 3     | QS `IdleInhibitor`, G, video |
| keyboard-shortcuts-inhibit-v1         | smithay | 3     | QS `ShortcutInhibitor`, G, VMs |
| ext-background-effect-v1              | smithay | 6     | QS `BackgroundEffect` (blur) |
| hyprland-focus-grab-v1                | own     | 3     | QS `HyprlandFocusGrab`, Noctalia popups |
| xdg-activation-v1                     | smithay | 3     | focus requests, notifications |
| wlr-data-control / ext-data-control   | smithay | 3     | clipboard managers, QS |
| xdg-foreign v1/v2                     | smithay | 3     | portals parent windows |
| xdg-toplevel-icon-v1                  | smithay | 3     | QS/taskbar icons       |
| xdg-toplevel-tag-v1                   | smithay | 6     | window rules matching  |
| xdg-dialog-v1                         | smithay | 6     | modal hints            |
| xdg-system-bell-v1                    | smithay | 6     | urgent border          |
| security-context-v1                   | smithay | 3     | flatpak sandboxing, rule matching |

## Outputs

| protocol                          | source  | phase | why                     |
| --------------------------------- | ------- | ----- | ----------------------- |
| wlr-output-management-v1          | own     | 3     | wlr-randr, kanshi       |
| wlr-output-power-management-v1    | own     | 3     | wlopm, idle display off |
| wlr-gamma-control-v1              | own     | 3     | wlsunset, gammastep     |
| color-management-v1               | own     | 7     | HDR (not in Smithay 0.7) |

## Gaming

| protocol                          | source  | phase | why                     |
| --------------------------------- | ------- | ----- | ----------------------- |
| pointer-constraints-v1            | smithay | 4     | mouse look (native + Xwayland) |
| relative-pointer-v1               | smithay | 4     | raw mouse motion        |
| pointer-warp-v1                   | smithay | 4     | cursor warps (Wine)     |
| linux-drm-syncobj-v1              | smithay | 4     | explicit sync, NVIDIA   |
| fifo-v1                           | smithay | 4     | Vulkan FIFO pacing      |
| commit-timing-v1                  | smithay | 4     | frame pacing            |
| wp-tearing-control-v1             | own     | 4     | async flips             |
| content-type-v1                   | smithay | 4     | `game` hint → policy    |
| drm-lease-v1                      | smithay | 4     | VR headsets             |
| alpha-modifier-v1                 | smithay | 4     | overlays (mangohud etc.) |

X11 goes through xwayland-satellite, which is itself the Wayland server
for Xwayland. So xwayland-shell-v1 and xwayland-keyboard-grab-v1 are not
needed on our side; satellite forwards what X11 games need (pointer
constraints, relative pointer, dmabuf) as normal Wayland requests.

## Input

| protocol                          | source  | phase | why                    |
| --------------------------------- | ------- | ----- | ---------------------- |
| pointer-gestures-v1               | smithay | 6     | touchpad gestures      |
| tablet-v2                         | smithay | 6     | drawing tablets        |
| text-input-v3, input-method-v2    | smithay | 6     | fcitx5 / ibus          |
| virtual-keyboard-v1               | smithay | 5     | end4-pC on-screen keyboard, wtype |
| virtual-pointer-v1                | smithay | 6     | wlrctl, remote desktop |

## Not planned

- wlr-export-dmabuf (superseded by ext-image-copy-capture)
- wl_drm (legacy Mesa; only add if something breaks without it)
- hyprland-global-shortcuts, hyprland-toplevel-export, hyprland-surface,
  hyprland-toplevel-mapping, Hyprland/i3/Sway IPC emulation
- KDE/GNOME private protocols
