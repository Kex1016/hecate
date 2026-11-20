# Layer rules for the bundled shell. The compositor owns motion and the look of the layers
# (pixelate transitions, frames, tint), so there is no blur and no per-panel animation here.
let
  ns = name: { namespace = "^quickshell:${name}$"; };
  rule = name: attrs: { match = ns name; } // attrs;
in
[
  # The lock screen is the wallpaper layer (with the clock) showing through the transparent lock
  # surface.
  (rule "background" { show_when_locked = true; })
]
