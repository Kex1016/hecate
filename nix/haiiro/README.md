# haiiro app templates

The fixed haiiro look for programs the shell does not draw: kitty, foot, GTK 3/4 and Qt (KDE
colour scheme), plus fontconfig's `monospace`. Rosé Pine (main) colours, the IBM VGA 8x16 face at
16px (12pt) with Unifont behind it, square corners. Nothing here is generated from a wallpaper.

The Home Manager module installs them with `shell.theming.apps` (on by default), see
`docs/nix.md`:

| file | where it goes | how it is picked up |
| --- | --- | --- |
| `kitty.conf` | `~/.config/kitty/haiiro.conf` | `include haiiro.conf` in kitty.conf (seeded once, or added to `programs.kitty`) |
| `foot.ini` | `~/.config/foot/haiiro.ini` | `include=` in foot.ini (seeded once, or added to `programs.foot`) |
| `gtk3.css`, `gtk4.css` | `~/.config/gtk-3.0/gtk.css`, `~/.config/gtk-4.0/gtk.css` | GTK 3 (with adw-gtk3-dark) and libadwaita; dconf gets the theme, dark style and fonts once |
| `Haiiro.colors` | `~/.local/share/color-schemes/Haiiro.colors` | `ColorScheme=Haiiro` in kdeglobals |
| `kdeglobals` | `~/.config/kdeglobals`, seeded once (writable) | Qt's KDE platform theme |
| `50-haiiro-mono.conf` | `~/.config/fontconfig/conf.d/` | puts the haiiro mono stack first for `monospace` |

Terminal colours follow Rosé Pine's own mapping: black overlay, red love, green pine, yellow
gold, blue foam, magenta iris, cyan rose, white text, bright black muted.
