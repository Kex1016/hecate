# haiiro for Cider

The haiiro theme for the Cider Apple Music client (Flatpak `sh.cider.Cider`): Rosé Pine, the IBM VGA
8x16 face with Jacquard 24 for page titles (always lowercase), square corners, no blur, no easing.
Cider is a Flatpak the flake does not manage, so this folder is not installed by the Home Manager
module. Copy it by hand or wire it from your own configuration.

| file | what it is |
| --- | --- |
| `theme.yml` | Cider theme manifest (name and the stylesheet list) |
| `main.css` | the whole theme, with the fonts inlined as data URIs |
| `settings.json` | the Cider settings the theme needs, as `CiderApp.config` paths |

The fonts are inlined because the Flatpak sandbox cannot see the host's fontconfig fonts. Only
Unifont (the fallback) is picked up from `/run/host/fonts`.

## Install

1. Quit Cider, then copy `theme.yml` and `main.css` to
   `~/.var/app/sh.cider.Cider/config/sh.cider.genten/themes/haiiro/`.
2. Start Cider and open Settings, then Extensions. Under Themes, open Haiiro and tick
   "Main Stylesheet". Cider reads the theme when it is toggled, so toggle it again after you update
   `main.css`.
3. Apply the settings below, or the theme fights Cider's artwork colouring (see Settings).

## Settings

Cider colours the UI from the playing artwork, rounds viewports and uses glass materials. The theme
overrides what CSS can reach, but these settings must be off for it to look right. They live in
`spa-config.yml` next to the themes folder (same keys, nested by dots). `settings.json` has the
wanted values.

| setting | haiiro | Cider default here |
| --- | --- | --- |
| `visual.useAdaptiveColors` | false | true |
| `visual.useEmotionEngine` | false | true |
| `visual.matchPageToArtwork` | false | true |
| `visual.matchPageGlow` | false | true |
| `visual.ui_custom.customAccentColor` | false | true |
| `visual.ui_custom.customTintColor` | false | true |
| `visual.transparency` | false | false |
| `visual.forceViewportRounding` | false | true |
| `visual.viewTransitions` | false | true |
| `components.Glaze.Type` | `"none"` | `"none"` |
| `components.MojaveLayouts.PlayerMaterial` | `"none"` | `"default"` |
| `components.MojaveLayouts.LiquidGlassPlayer` | false | true |
| `components.MojaveLayouts.LCDChromaticAberration` | false | false |
| `components.MojaveLayouts.FloatingPlayerRadius` | 0 | 13 |

To restore, set the right column back. Keep a copy of `spa-config.yml` before editing it, and edit
it with Cider closed (Cider rewrites it on exit).

## Notes

- Native menus (the "..." menu and context menus) are drawn by the OS, not the page, so the theme
  cannot style them.
- Artwork, Apple's category tiles and Apple's own badges keep their colours.
- Dialogs, tooltips and toasts are styled from Cider's markup but were not seen live.
- Developed over remote debugging against a copy of the profile: quit Cider and run
  `flatpak run sh.cider.Cider --user-data-dir=<copy> --remote-debugging-port=9224`. Themes load from
  the real data directory even with a copy.
