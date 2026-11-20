pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs
import qs.services
import qs.modules.common

// What the quick panel remembers while it is closed: which list a long-pressed
// chip opened, the bottom tab, and Game Mode.
//
// Game Mode turns motion off. It sets Appearance.gameMode, which makes every
// shell transition a cut (Appearance.haiiro.transition.cut), and tells the
// compositor to stop animating and allow tearing. Turning it off unsets those
// runtime overrides, so the user's own config comes back.
Singleton {
    id: root

    // "" (the panel), or the list a chip opened in place:
    // "wifi", "bluetooth", "vpn", "nightLight", "sink", "source"
    property string view: ""
    // "calendar", "todo" or "timer"
    property string tab: "calendar"

    property bool gameMode: false

    // hecate: set one by one, so a key a build doesn't know can't block the rest.
    readonly property var hecateOverrides: [
        ["animations.enable", "false"],
        ["gaming.allow_tearing", "\"always\""],
    ]

    function openView(name) {
        root.view = name;
        GlobalStates.sidebarRightOpen = true;
    }

    function setGameMode(on) {
        root.gameMode = on;
        if (WM.compositor === "hecate") {
            const cmds = root.hecateOverrides.map(([path, value]) => on ? `hecate msg set ${path} '${value}'` : `hecate msg unset ${path}`);
            Quickshell.execDetached(["bash", "-c", cmds.join("; ")]);
        } else if (WM.compositor === "niri") {
            // niri has no runtime switch for animations; the shell side still goes still.
        } else if (on) {
            Quickshell.execDetached(["hyprctl", "--batch", "keyword animations:enabled 0; keyword general:allow_tearing 1"]);
        } else {
            Quickshell.execDetached(["hyprctl", "reload"]);
        }
    }

    function toggleGameMode() {
        root.setGameMode(!root.gameMode);
    }

    Binding {
        target: Appearance
        property: "gameMode"
        value: root.gameMode
    }

    // Pick up a Game Mode that outlived a shell restart: animations off in the compositor.
    Process {
        id: fetchGameMode
        running: WM.compositor === "hecate" || WM.compositor === "hyprland"
        command: WM.compositor === "hecate"
            ? ["bash", "-c", "test \"$(hecate msg -j get animations.enable)\" = false"]
            : ["bash", "-c", "test \"$(hyprctl getoption animations:enabled -j | jq '.int')\" -eq 0"]
        onExited: (exitCode, exitStatus) => root.gameMode = exitCode === 0
    }

    // `quickPanel list wifi` opens the panel on a chip's list (wifi, bluetooth,
    // vpn, nightLight, sink, source); `quickPanel tab todo` picks a bottom tab.
    IpcHandler {
        target: "quickPanel"

        function list(name: string): void {
            root.openView(name);
        }
        function tab(name: string): void {
            root.tab = name;
            GlobalStates.sidebarRightOpen = true;
        }
    }

    IpcHandler {
        target: "gameMode"

        function toggle(): void {
            root.toggleGameMode();
        }
        function enable(): void {
            root.setGameMode(true);
        }
        function disable(): void {
            root.setGameMode(false);
        }
        function isOn(): bool {
            return root.gameMode;
        }
    }
}
