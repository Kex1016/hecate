# Animations

Decided in round 3: add the animations the bundled shell needs. This breaks
from labwc's no-animation stance on purpose. Everything is configurable,
can be turned off globally, and turns itself off for games.

Source: end-4's `dots-hyprland/dots/.config/hypr/hyprland/general.lua`
(`hl.animation` / `hl.curve`) and `rules.lua` (`hl.layer_rule ...
animation`), plus end4-pC's Game Mode toggle.

## What we animate (v1)

| animation | end-4 setting | hecate default | notes |
| --------- | ------------- | -------------- | ----- |
| window open | `windowsIn` popin 80% + `fadeIn`, emphasizedDecel, speed 3 | `window_open = { style = "popin", from = 0.8, curve = "emphasized_decel", ms = 300 }` | scale + fade from the window centre |
| window close | `windowsOut` popin 90% + `fadeOut`, speed 2 | `window_close = { style = "popin", from = 0.9, ms = 200 }` | needs a snapshot of the last frame (the client has already unmapped). Smithay renders the surface tree to a texture on unmap; niri does the same |
| window move/resize | `windowsMove` slide, speed 3 | `window_move = { style = "slide", ms = 300 }` | compositor-driven geometry changes only: snap, maximize, unmaximize, keyboard move, size presets. Interactive drags stay 1:1 with the pointer. Resizes crossfade old and new buffers until the client commits the new size |
| border colour | `border`, speed 10 (= 1 s) | `border = { ms = 1000 }` | active/inactive colour fade in the border shader |
| layer open | `layersIn` popin 93% + `fadeLayersIn` | `layer_open = { style = "popin", from = 0.93, ms = 270 }` | per-namespace override via layer rules |
| layer close | `layersOut` popin 94% + `fadeLayersOut` (stall curve) | `layer_close = { style = "popin", from = 0.94, ms = 240 }` | snapshot like window close |
| layer per-rule styles | `slide`, `slide left`, `slide right`, `slide bottom`, `popin 120%`, `no_anim` | layer rule `animation = "slide" \| "slide_left" \| "slide_right" \| "slide_top" \| "slide_bottom" \| { popin = 1.2 } \| "none"` | `slide` without a direction picks the nearest anchored edge, like Hyprland |
| workspace switch | `workspaces` slide, menu_decel, speed 7 | `workspace_switch = { style = "slide", ms = 700 }` | horizontal slide of the workspace layer per output (`per_output` scope) or on all outputs (`global`). Also `"fade"` and `"none"` |

Not animated: `specialWorkspace*` (hecate has no scratchpad workspace) and
`zoomFactor` (no zoom in v1).

Hyprland's `speed` is in deciseconds (speed 3 = 300 ms). Our config uses
milliseconds directly.

## Curves

Named cubic béziers, the same model as Hyprland (`hl.curve`), plus
`linear`. Built-ins match end-4's names so the shell profile is a direct
translation:

```toml
[animations.curves]
emphasized_decel = [0.05, 0.7, 0.1, 1.0]
emphasized_accel = [0.3, 0.0, 0.8, 0.15]
standard_decel = [0.0, 0.0, 0.0, 1.0]
menu_decel = [0.1, 1.0, 0.0, 1.0]
menu_accel = [0.52, 0.03, 0.72, 0.08]
expressive_fast_spatial = [0.42, 1.67, 0.21, 0.90]
expressive_slow_spatial = [0.39, 1.29, 0.35, 0.98]
expressive_default_spatial = [0.38, 1.21, 0.22, 1.00]
stall = [1.0, -0.1, 0.7, 0.85]
```

Overshooting curves (y outside 0..1) are allowed. Spring physics like
niri's are a later option.

## Config shape

```toml
[animations]
enable = true
slowdown = 1.0                 # global multiplier, 0 < x; handy for debugging

[animations.window_open]
style = "popin"                # "popin" | "fade" | "slide" | "none"
from = 0.8
ms = 300
curve = "emphasized_decel"
fade = true

# ... window_close, window_move, border, layer_open, layer_close, workspace_switch

[[layer_rules]]
match = { namespace = "quickshell:bar" }
animation = "slide"
```

## Gaming and performance rules

- **No animation while a fullscreen window is on an output.** That output
  stays on the direct scanout path, which animations would break.
- Windows and layers whose content type is `game`, or that match a
  `no_animations` window rule, never animate.
- Animations run on the output's frame clock (presentation feedback), not a
  timer, so they don't add extra frames on VRR outputs when idle.
- Reduced motion: `animations.enable = false` snaps everything instantly;
  the code path is the same with zero duration.

## Game Mode (runtime overrides)

end4-pC's Game Mode quick toggle runs, on Hyprland:
`hyprctl --batch "keyword animations:enabled 0; keyword
decoration:shadow:enabled 0; keyword decoration:blur:enabled 0; keyword
general:gaps_in 0; keyword general:gaps_out 0; keyword
general:border_size 1; keyword decoration:rounding 0; keyword
general:allow_tearing 1"`, and checks state with `hyprctl getoption
animations:enabled`.

hecate gets a general mechanism for this instead of a Game Mode feature:
**runtime overrides** over IPC.

```
hecate msg set animations.enable false
hecate msg set blur.enable false
hecate msg set border.width 1
hecate msg set border.radius 0
hecate msg set general.gaps 0
hecate msg set gaming.allow_tearing always
hecate msg get animations.enable
hecate msg unset --all        # back to the config file
```

- Overrides sit on top of the loaded config, validated with the same
  schema, and survive config reloads. They're never written to disk, so
  your Nix config stays the source of truth after a restart.
- The shell's `GameMode.qml` gets a hecate branch running those `set`
  commands, and `get` for the toggle state.
- `{"set": {"path": ..., "value": ...}}`, `{"get": ...}`, `{"unset": ...}`
  in the IPC (see [ipc.md](ipc.md)).

## Implementation notes

- `render/anim.rs`: an `Animation` value (start, duration, curve) sampled at
  each output's predicted presentation time. Window, layer and workspace
  state hold optional animations; the render element list applies
  transforms (scale, offset, alpha) while building the frame.
- Close animations keep a texture snapshot plus geometry after the surface
  is gone, and render it until done.
- Popin/slide are transforms on the whole surface tree including the
  rounded border and clip, so corners stay correct mid-animation.
- Blur under an animating layer (xray mode) is just a cropped region of the
  cached blurred wallpaper, so it stays cheap.
