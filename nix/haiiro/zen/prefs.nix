# Zen prefs the haiiro userChrome.css and userContent.css rely on, for a
# Home Manager profile's `settings`. haiiro is dark only.
{
  "toolkit.legacyUserProfileCustomizations.stylesheets" = true;

  # Dark chrome and dark internal pages, whatever the system says
  "zen.view.window.scheme" = 0;
  "layout.css.prefers-color-scheme.content-override" = 0;
  # Pages that set no colours of their own
  "browser.display.background_color.dark" = "#191724";
  "browser.display.foreground_color.dark" = "#e0def4";

  # Square corners and no gap around the page; the frame comes from the CSS
  "zen.theme.border-radius" = 0;
  "zen.theme.content-element-separation" = 0;
  # No blurred backdrop behind the floating URL bar
  "zen.theme.acrylic-urlbar" = false;

  # Cuts instead of slides. Web pages keep their own prefers-reduced-motion;
  # the rest of Zen's motion is cut by motion.cfg.js
  "zen.workspaces.switch-animation-duration" = 1;
  "zen.glance.animation-duration" = 0;
  "zen.view.compact.animate-sidebar" = false;
  "zen.watermark.enabled" = false;

  # Solid square scrollbars, the thumb as wide as the track
  "widget.gtk.overlay-scrollbars.enabled" = false;
  "widget.non-native-theme.gtk.scrollbar.round-thumb" = false;
  "widget.non-native-theme.gtk.scrollbar.thumb-size" = "1.0";
}
