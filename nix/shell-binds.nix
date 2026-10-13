# Keybind profile for the bundled shell (docs/keybinds.md, "shell" layer). end-4's Hyprland
# global shortcuts become calls into the shell's `shortcut` IPC target.
{ shell }:
let
  ipc = "${shell}/bin/hecate-shell-ipc";
  trigger = name: {
    spawn = [
      ipc
      "shortcut"
      "trigger"
      name
    ];
  };
  hold = name: {
    on_press.spawn = [
      ipc
      "shortcut"
      "press"
      name
    ];
    on_release.spawn = [
      ipc
      "shortcut"
      "release"
      name
    ];
    transparent = true;
  };
  sh = cmd: { spawn_sh = cmd; };
in
{
  # Tap Super for search; hold it to show workspace numbers.
  "Super".tap.spawn = [
    ipc
    "shortcut"
    "trigger"
    "searchToggleRelease"
  ];
  "Super_L" = hold "workspaceNumber";
  "Super_R" = hold "workspaceNumber";

  # The Alt+Tab switcher (modules/ii/altTab): signals on hecate's event stream rather than a
  # process per key press. Releasing Alt switches; the release still reaches the window.
  "Alt+Tab".signal = "alt-tab-next";
  "Alt+Shift+Tab".signal = "alt-tab-prev";
  "Alt+Escape".signal = "alt-tab-cancel";
  "Alt_L" = {
    on_release.signal = "alt-tab-commit";
    transparent = true;
  };
  "Alt_R" = {
    on_release.signal = "alt-tab-commit";
    transparent = true;
  };

  "Mod+V" = trigger "overviewClipboardToggle";
  "Mod+Period" = trigger "overviewEmojiToggle";
  "Mod+A" = trigger "sidebarLeftToggle";
  "Mod+O" = trigger "sidebarLeftToggle";
  "Mod+Alt+A" = trigger "sidebarLeftToggleDetach";
  "Mod+N" = trigger "sidebarRightToggle";
  "Mod+K" = trigger "oskToggle";
  "Mod+M" = trigger "mediaControlsToggle";
  "Mod+G" = trigger "overlayToggle";
  "Mod+J" = trigger "barToggle";
  "Ctrl+Alt+Delete" = trigger "sessionToggle";
  "Mod+Ctrl+T" = trigger "wallpaperSelectorToggle";
  "Mod+Ctrl+Alt+T" = trigger "wallpaperSelectorRandom";
  "Mod+Escape" = trigger "settingsToggle";

  "Mod+Shift+S" = trigger "regionScreenshot";
  "Print" = trigger "regionScreenshot";
  "Mod+Shift+A" = trigger "regionSearch";
  "Mod+Shift+X" = trigger "regionOcr";
  "Mod+Shift+T" = trigger "screenTranslate";
  "Mod+Shift+R" = trigger "regionRecord";
  "Mod+Alt+R" = trigger "regionRecord";
  "Mod+Shift+C" = sh "hyprpicker -a";

  "Mod+L" = {
    spawn = [
      ipc
      "lock"
      "activate"
    ];
    allow_when_locked = true;
  };
  "Mod+Ctrl+R" = sh "systemctl --user restart hecate-shell";

  # Window keys that the shell profile reassigns (docs/keybinds.md, conflicts).
  "Mod+F" = "toggle_fullscreen";
  "Mod+D" = "toggle_maximize";

  "Mod+Shift+N" = sh "playerctl next";
  "Mod+Shift+B" = sh "playerctl previous";
  "Mod+Shift+P" = sh "playerctl play-pause";
  "Mod+Shift+M" = sh "wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle";
  "Mod+Alt+M" = sh "wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle";
}
