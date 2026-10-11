# Layer rules for the bundled shell, after end-4's Hyprland ones: blur behind the panels
# (only where they're mostly opaque) and per-panel open/close animations.
let
  ns = name: { namespace = "^quickshell:${name}$"; };
  rule = name: attrs: { match = ns name; } // attrs;
in
[
  {
    match.namespace = "^quickshell:.*";
    blur = true;
    blur_popups = true;
    ignore_alpha = 0.79;
    xray = true;
  }
  # The wallpaper and the screen frame are what the rest blur.
  (rule "(background|wallpaper|screenframe.*|screenCorners)" {
    blur = false;
    animation = "none";
  })
  (rule "bar" { animation = "slide"; })
  (rule "verticalBar" { animation = "slide"; })
  (rule "sidebarLeft" { animation = "slide_left"; })
  (rule "sidebarRight" { animation = "slide_right"; })
  (rule "(dock|osk)" { animation = "slide_bottom"; })
  (rule "notificationPopup" { animation = "fade"; })
  (rule "(onScreenDisplay|mediaControls|equalizer)" { animation = "fade"; })
  # Things that must show up at once.
  (rule "(regionSelector|overlay|reloadPopup)" {
    animation = "none";
  })
  (rule "session" {
    animation = "fade";
    ignore_alpha = 0.0;
  })
  (rule "overview" { animation.popin = 0.95; })
]
