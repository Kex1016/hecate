# Layer rules for the bundled shell. The compositor owns the look of the layers (frames, tint),
# so there is no blur here. The shell's panels, toasts and OSD pixelate in and out themselves
# (PixelTransition), so the compositor's own layer pixelate is off for every shell layer:
# with both, each panel would pixelate twice.
let
  ns = name: { namespace = "^quickshell:${name}$"; };
  rule = name: attrs: { match = ns name; } // attrs;
in
[
  (rule ".*" {
    animation = "none";
    blur = false;
  })
  # The lock screen is the wallpaper layer (with the clock) showing through the transparent lock
  # surface.
  (rule "background" { show_when_locked = true; })
]
