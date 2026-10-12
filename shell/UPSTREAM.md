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
- Hyprland-only features hidden or rerouted on hecate (night light, keybind list, game mode,
  updater)
