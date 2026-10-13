# Bundled shell: upstreams

Both directories are `git subtree --squash` imports. Our changes are ordinary commits on top
(`feat(shell): ...` / `fix(shell): ...`), so `git log -- shell/` lists them.

| directory        | upstream                                   | pinned    |
| ---------------- | ------------------------------------------ | --------- |
| `end4-pC/`       | https://github.com/pctrade/end4-pC (`main`) | `26fe30f` |
| `dots-hyprland/` | https://github.com/end-4/dots-hyprland (`main`), pruned | `2f2a376` |

Both are GPL-3.0; their `LICENSE` files (and dots-hyprland's `licenses/`) stay in place.

## Updating end4-pC

```sh
git subtree pull --prefix=shell/end4-pC https://github.com/pctrade/end4-pC main --squash \
  -m "chore(shell): update end4-pC to <rev>"
```

Conflicts land in files we patched; resolve them keeping upstream's change plus our hecate
branch. Update the pin above.

## Updating dots-hyprland

Only part of the repo is vendored. Rebuild the pruned tree from a fresh clone, then pull it:

```sh
git clone --depth 1 https://github.com/end-4/dots-hyprland /tmp/dh && cd /tmp/dh
git checkout --orphan pruned && git rm -rq --cached .
git add dots/.config/{matugen,Kvantum,kde-material-you-colors,fontconfig,kitty,foot,fuzzel,fish,kdeglobals} \
        sdata/uv sdata/dist-nix sdata/deps-info.md licenses LICENSE
git commit -m "Pruned dots-hyprland <rev> for hecate"
cd - && git subtree pull --prefix=shell/dots-hyprland /tmp/dh pruned --squash \
  -m "chore(shell): update dots-hyprland to <rev>"
```

Left out on purpose: the Hyprland config (hecate's config replaces it), the `ii` QuickShell
config (end4-pC is the shell), the per-distro install scripts and the other app dotfiles.

## Our patches

See `git log -- shell/end4-pC` for the full list. In short:

- hecate backend for the compositor abstraction (`services/WM.qml`, `HecateBackend.qml`,
  `HecateXkb.qml`)
- global shortcuts through an IPC registry instead of Hyprland's `GlobalShortcut`
- focus grabs enabled on hecate (hyprland-focus-grab-v1)
- theming without Plasma: Qt colour schemes applied through kdeglobals, Papirus folders in
  the wallpaper's colour, a matugen template for hecate's border colours
- per-monitor wallpapers (Settings → Background → Per monitor)
- the lock screen shows the blurred wallpaper and clock (through a `show_when_locked` layer
  rule) rather than the niri fallback
- Boot Windows (password prompt, then `~/.local/bin/boot-windows` as root) in place of
  Task Manager in the session menu
- idle lock, screens off and sleep driven by the shell (ext-idle-notify) instead of hypridle
- app theming templates (foot, kitty, user templates), after Noctalia's; terminal colours
  work again (applycolor.sh no longer exits without ~/.config/quickshell/ii)
- on CakeOS, an update check against upstream CakeOS and the shell's own updater
- an Alt+Tab switcher (most recently used order, live window previews)
- the login screen (hecate-greetd): `LockSurface` driven by a greetd context with a user and
  session picker, `GreeterSync` sharing the look with it, Settings → hecate-greetd
- a screen sharing picker (screens and windows, live previews) for xdg-desktop-portal-wlr
- an image picker for the Custom Image and Image Card desktop widgets, which upstream only
  set by dropping a file on them
- faster startup: the startup-lock flag clears as soon as the config says no lock is due
  (`GlobalStates.settleStartupLock`), and panels load asynchronously, visible ones first
  (`PanelLoader`, `IllogicalImpulseFamily`)
- Hyprland-only features hidden or rerouted on hecate (night light, keybind list, game mode,
  updater)
