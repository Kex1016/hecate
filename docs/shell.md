# Bundled shell: illogical-impulse + end4-pC

hecate ships with a desktop shell in the same repo: end-4's
[illogical-impulse](https://github.com/end-4/dots-hyprland) dotfiles, with
pctrade's [end4-pC](https://github.com/pctrade/end4-pC) QuickShell config as
the shell. Everything gets patched to run on hecate and packaged so
`nixos-rebuild` gives a working desktop with no manual setup.

Notes from reading both repos (dots-hyprland `2f2a376`, end4-pC `main`,
October 2026).

## Licensing

Both are **GPL-3.0** (dots-hyprland bundles some LGPL-3.0/MIT pieces,
listed in its `licenses/`). That's compatible with hecate's
GPL-3.0-or-later. Vendoring is fine even if the repo goes public later; we
keep their LICENSE files, copyright and credits next to the code.

## What each piece is

- **end4-pC**: a fork of illogical-impulse's QuickShell config (`ii`):
  ~700 QML/JS/shell/Python files, launched with `qs -c end4-pC`. It
  "requires illogical-impulse to be installed": in practice that means the
  dependency set (fonts, matugen, kde-material-you-colors, Python env,
  cliphist, etc.) and a few dotfiles, not ii's QML.
- **dots-hyprland**: the illogical-impulse dotfiles: Hyprland config (Lua),
  QuickShell `ii`, matugen templates, Kvantum/KDE colour configs, kitty,
  foot, fuzzel, fish, fontconfig, the dependency lists per distro, a WIP
  `sdata/dist-nix` Home Manager flake (wraps QuickShell with its Qt deps),
  and a Python `requirements.in` for the colour/wallpaper scripts.

We use end4-pC as the shell, plus from dots-hyprland: the dependency list,
the non-Hyprland dotfiles (matugen, Kvantum, kde-material-you-colors,
fontconfig, kitty/foot/fuzzel/fish), the Python requirements and the
dist-nix wrapper as a starting point. The Hyprland config is dropped:
hecate's config replaces it.

## How Hyprland-bound end4-pC is

Measured with grep over end4-pC:

| usage                                   | count |
| --------------------------------------- | ----- |
| files mentioning Hyprland / hyprctl     | 105 of ~700 |
| `import Quickshell.Hyprland`            | 49 files |
| `GlobalShortcut` (Hyprland-only)        | 50 uses, 45 shortcut names |
| `Hyprland.dispatch(...)`                | 24 |
| `Hyprland.focusedMonitor` / `.monitors` / `.monitorFor` | 32 |
| `HyprlandFocusGrab`                     | 7 |
| `hyprctl ...` in scripts and QML        | ~25 (monitors -j, dispatch, cursorpos, hyprsunset, getoption, reload, setcursor) |
| existing `WM.compositor` checks         | 47 for hyprland, 26 for niri |

The good news: end4-pC already has a **compositor abstraction**.
`services/WM.qml` detects the compositor from `XDG_CURRENT_DESKTOP` and
loads `HyprlandBackend.qml` or `NiriBackend.qml`. Both expose the same
interface: `windowList`, `workspaces`, `activeWorkspace`, `monitors`,
`focusedMonitor`, `focusWindow`, `closeWindow`, `switchWorkspace`,
`moveWindowToWorkspace`, `monitorFor`, `fullscreenOnMonitor`, ...
`NiriBackend.qml` is 160 lines: it shells out to `niri msg -j
windows|workspaces|outputs`, follows `niri msg -j event-stream`, and
normalises the JSON. There are also `NiriXkb.qml` and `NiriConfig.qml`
counterparts.

So the port is mostly "add a hecate backend next to the niri one", not a
rewrite. Unknown compositors currently fall back to the **Hyprland**
backend, which is why it has to be explicit.

## Port plan

### Compositor side (makes the shell port small)

1. **IPC JSON shaped like niri's** for `windows`, `workspaces`, `outputs`
   and `event-stream` (field names `id`, `app_id`, `title`,
   `workspace_id`, `is_focused`, `is_active`, `output`, `logical {x, y,
   width, height, scale}`, `layout.window_size`). Then `HecateBackend.qml`
   starts as a copy of `NiriBackend.qml` with `hecate msg` and a handful
   of differences. See [ipc.md](ipc.md).
2. **hyprland-focus-grab-v1** implemented, so `GlobalFocusGrab.qml` works
   unchanged once its `active:` guard includes hecate (sidebars and menus
   close on outside click).
3. **Press and release binds**: end-4 uses `GlobalShortcut` `pressed` /
   `released` (hold Super to show workspace numbers, tap Super to open
   search). hecate binds support `on_press` + `on_release` actions and
   modifier-only taps (fires on release only if no other key was pressed).
   See [keybinds.md](keybinds.md).
4. **Layer rules with blur**: end-4 gets its frosted panels from Hyprland
   layer rules (`blur`, `ignore_alpha`, `blur_popups`, global `xray`), not
   from the client asking. hecate implements the same rules; the shell
   profile ships end-4's set translated from `hypr/hyprland/rules.lua`.
5. **Animations** matching end-4's Hyprland settings (window popin/fade,
   layer popin and per-namespace slides, workspace slide, border fade) and
   **runtime overrides** for the Game Mode toggle. See
   [animations.md](animations.md).
6. IPC extras the shell needs: `cursor_position`, `keyboard_layouts`
   (+ switch), `binds` (cheat sheet), `set_cursor_theme`, output power
   on/off, `reload`.

### Shell side (patches, kept as small, reviewable commits)

| area | change |
| ---- | ------ |
| `services/WM.qml` | detect `hecate` (`XDG_CURRENT_DESKTOP` / `HECATE_SOCKET`), load `HecateBackend` |
| `services/HecateBackend.qml` | new, modelled on `NiriBackend.qml` |
| `services/HecateXkb.qml` | new, modelled on `NiriXkb.qml`, uses `hecate msg -j keyboard-layouts` |
| `services/CompositorGlobalShortcut.qml` | on hecate, register into a registry instead of creating a `GlobalShortcut` |
| new `IpcHandler { target: "shortcut" }` | `press(name)` / `release(name)` fire any registered shortcut. hecate binds call `qs -c end4-pC ipc call shortcut press searchToggleRelease`. Covers all 45 names with one patch instead of 45 |
| `services/GlobalFocusGrab.qml` | `active` also on hecate |
| direct `Hyprland.focusedMonitor` / `monitorFor` uses | route through `WM.*` (already exists) where not yet done |
| `hyprctl monitors -j`, `cursorpos`, `dispatch` in scripts | small `scripts/wm/*` helpers that branch on compositor, hecate → `hecate msg` |
| `Hyprsunset.qml` | on hecate drive `wlsunset` / `gammastep` (wlr-gamma-control, which hecate implements) |
| `HyprlandAntiFlashbangShader` | hidden on hecate (Hyprland screen shader, no equivalent) |
| `HyprlandKeybinds.qml` / cheat sheet | on hecate read `hecate msg -j binds` |
| Settings → Hyprland config page | replaced on hecate by a page that writes `~/.config/hecate/shell.toml`, which the Nix-generated config `include`s. Nix config stays read-only; shell tweaks go in that file. Besides window, focus and effect settings it has **Displays** (a drag-to-arrange map that snaps monitors edge to edge, and per output mode/refresh rate from `hecate msg outputs`, scale, orientation, VRR, position) and **Mouse** (acceleration profile and speed for `input."type:pointer"`); hecate applies all of it live on reload. Settings for a monitor that isn't plugged in stay in the file and apply when it returns |
| `GameMode.qml` (`hyprctl --batch keyword ...`, `getoption animations:enabled`) | hecate branch using runtime overrides: `hecate msg set animations.enable false`, `blur.enable false`, `border.radius 0`, `gaming.allow_tearing always`, ...; state from `hecate msg get` ([animations.md](animations.md#game-mode-runtime-overrides)) |
| `scripts/colors/switchwall.sh` | without `plasma-apply-colorscheme`, apply the Qt colour scheme with `apply_kde_colors.py` (kdeglobals + D-Bus signal); recolour Papirus's folders with `folder_icons.py` |
| matugen `templates/hecate/colors.toml` (dots-hyprland) | new: hecate's border colours, included by the Nix config |
| per-monitor wallpapers (`background.monitorWallpapers`, Background, Wallpapers, wallpaper selector, Settings → Background → Per monitor) | new: a monitor can have its own image; the rest show the main wallpaper, which alone drives the colour theme |
| Custom Image / Image Card widgets (Settings → Background) | new: a "Choose…" row picks the image (kdialog, else zenity); upstream could only drop a file on the widget. Dropped paths are URL-decoded, so names with spaces work |
| lock screen (`LockSurface`, `shell-layer-rules.nix`) | Hyprland shows the wallpaper layer, zoomed and blurred with the clock, through the transparent lock surface (`session_lock_xray`). On hecate the `quickshell:background` layer rule sets `show_when_locked`, and the niri fallback (an unblurred wallpaper drawn by the lock surface) is off |
| login screen ([hecate-greetd](https://github.com/Kex1016/hecate-greetd); `LockSurface`, `services/GreeterSync.qml`, Settings → hecate-greetd) | the greetd greeter runs this lock screen inside hecate, with the look of whoever logs in. `LockSurface` takes any context with `LockContext`'s interface (the greeter's talks to greetd: secret and visible prompts, its failure text) and a `userComponent` for the user and session picker; power buttons need no password there. `GlobalStates.greeterMode` keeps the wallpaper layer below the password surface, `lockStatusText` replaces "Locked". The shell runs `hecate-greetd sync` at startup and after config, colour, avatar or hecate config changes, which copies config, colours, wallpapers, avatar, monitor layout and keyboard/mouse settings to `/var/lib/hecate-greetd/users/$USER`. `Config.options.greeter` holds the page's settings (share, style: lock/centered/blurred, wallpaper, displays, input, status text, toolbars) |
| session menu (`SessionScreen`, `BootWindowsDialog`) | Task Manager is replaced by **Boot Windows**: a password dialog in the shell's style runs `apps.bootWindows` (default `~/.local/bin/boot-windows`) through `sudo -S` with the user's PATH. A wrong password shows in the dialog |
| idle (`modules/ii/idle/HecateIdle.qml`, Settings → hecate → Idle) | upstream hands `hyprland.idle` (lock, screen off, standby) to hypridle. On hecate the shell acts on them itself through ext-idle-notify: lock, `hecate msg action power_off_outputs`, `systemctl suspend` (locking first). Idle inhibitors hold them off. A monitor is recreated when its time changes, since IdleMonitor ignores later `timeout` changes |
| app theming (`scripts/colors/hecate_templates.py`, `templates/`, Settings → hecate → App theming) | after Noctalia: built-in templates (foot, kitty) render the Material colours into a file and add an `include` to the app's config (read-only Home Manager configs are left alone, with a warning). User templates come from `~/.config/hecate/templates.toml` (`input_path`, `output_path`, `post_hook`, `{{ colors.<name>.default.<format> }}` with filters). `applycolor.sh` no longer exits when `~/.config/quickshell/ii` is missing, which had stopped all terminal theming |
| screen sharing picker (`modules/ii/sharePicker/SharePicker.qml`, `hecate-share-picker`) | xdg-desktop-portal-wlr runs `hecate-share-picker` (`chooser_type=simple`), which calls `sharePicker.pick <fifo>` and prints the answer: `Monitor: <output>`, `Window: hecate-<id as 16 hex digits>` (the ext-foreign-toplevel-list identifier), or nothing. The shell shows a dialog with live previews, screens and windows split by a connected button group, and Cancel / Share. Without the shell, the Home Manager module keeps fuzzel (screens only) |
| Alt+Tab switcher (`modules/ii/altTab/AltTab.qml`, `nix/shell-binds.nix`) | new: every window in most recently used order (`focus_serial` from `hecate msg windows`), live previews, the first Alt+Tab selecting the previous window. hecate binds send `signal`s over the event stream (`alt-tab-next`, `-prev`, `-commit` on Alt release, `-cancel`); the switcher never takes the keyboard. A quick tap switches without showing it |
| window previews (`HecateBackend.toplevelFor`) | Quickshell captures windows only through hyprland-toplevel-export-v1, which hecate now implements; hecate windows are matched to Quickshell toplevels by app id, title and creation order |
| `DotsUpdater` / `Updates` (git pull into `~/.config`) | disabled in the Nix build, updates come from the flake |
| updates on CakeOS (`Updates`, `modules/ii/cakeosUpdater`) | when `/etc/cakeos` holds the flake and `update-cakeos` exists, the update check is `update-cakeos check --json` (upstream commits not installed yet) and the bar widget, the dashboard and Settings → About open the shell's own updater: what's new, apply on next restart or now, the password, then `update-cakeos apply` through `sudo -S` with its log. The update runs in the `Updates` service, so hiding the dialog doesn't stop it |
| `ConflictKiller` (kills other bars/notification daemons) | review; keep but make it Nix-aware |

### Things that keep working without patches

Bar, sidebars, notifications, media, tray, audio (PipeWire), bluetooth,
network, battery, polkit, PAM lock screen, launcher, clipboard (cliphist),
wallpaper and Material You theming (matugen), screenshots and OCR (through
grim/slurp and ext/wlr screencopy), session lock (ext-session-lock),
idle (ext-idle-notify), layer-shell surfaces, on-screen keyboard
(virtual-keyboard).

## Repo layout

```
shell/
  end4-pC/               git subtree of pctrade/end4-pC, our patches as commits on top
  dots-hyprland/         git subtree of end-4/dots-hyprland (pruned: no Hyprland config, no ii QML, no install scripts)
  UPSTREAM.md            pinned upstream commits, how to update, list of our patches
```

**Why subtree, not patch files**: you asked for the changes applied and
bundled. With a subtree the patched code is plain files in the repo, and
`git subtree pull --squash` merges upstream updates with normal conflict
resolution. Patch files on top of a flake input would rot on every upstream
change to a patched file, and end4-pC moves fast.

**Pruning** dots-hyprland keeps the repo smaller (agreed in round 3: keep
whatever we need, add more later if a feature needs it). Subtree-split only what we
package (`dots/.config/{matugen,Kvantum,kde-material-you-colors,fontconfig,kitty,foot,fuzzel,fish}`,
`sdata/uv`, `sdata/dist-nix`, `sdata/deps-info.md`, `licenses/`, `LICENSE`).

## Nix packaging

See [nix.md](nix.md#bundled-shell). Short version:

- `packages.hecate-shell`: end4-pC in the store, plus a `hecate-shell`
  launcher that runs QuickShell with the right Qt modules
  (qt5compat, qtpositioning, qtmultimedia, qtvirtualkeyboard, kirigami,
  syntax-highlighting, ...), `PATH` set to every runtime tool (matugen,
  cliphist, grim, slurp, jq, brightnessctl, ddcutil, playerctl, cava,
  wlsunset, translate-shell, tesseract, ...), and a Python env built from
  `sdata/uv/requirements.in` (several, like `materialyoucolor` and
  `kde-material-you-colors`, need packaging ourselves).
- QuickShell pinned via its flake input: ii tracks quickshell-git.
- Mutable state stays in the usual places: `~/.config/illogical-impulse/`
  (shell settings, written by the settings UI), `~/.local/state/quickshell`,
  `~/.cache`. The store copy is read-only; anything the shell writes goes
  there.
- Home Manager `wayland.windowManager.hecate.shell.enable = true`: installs
  the package, the dotfiles above (matugen templates etc.), fonts, cursor and
  GTK/Qt theme settings, starts the shell as a user service on
  `hecate-session.target`, and adds the shell's keybinds to hecate's config.

## Other shells

QuickShell configs in general and Noctalia still work through the standard
protocols (see PLAN.md section 6); they're just not bundled.

## Risks

- end4-pC is a fast-moving personal fork; merges will conflict sometimes.
  Keeping our patches small and mostly additive (new files plus guarded
  branches) limits that.
- Some features are network services (AI chat, booru, Google Lens, preset
  gallery, Claude usage). They work as upstream does; nothing to port, but
  they need their runtime deps on `PATH`.
- The shell's Python bits use pip/uv upstream; reproducing that env in Nix
  is the largest single packaging task.
