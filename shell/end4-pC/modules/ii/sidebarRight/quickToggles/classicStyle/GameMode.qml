import qs.modules.common
import qs.modules.common.widgets
import qs.services
import Quickshell
import Quickshell.Io

QuickToggleButton {
    id: root
    buttonIcon: "gamepad"
    toggled: toggled

    // hecate: runtime overrides, set one by one so a key a build doesn't know can't block the rest.
    readonly property var hecateOverrides: [
        ["animations.enable", "false"],
        ["border.width", "1"],
        ["border.radius", "0"],
        ["gaming.allow_tearing", "\"always\""],
    ]

    onClicked: {
        root.toggled = !root.toggled
        if (WM.compositor === "hecate") {
            const cmds = root.hecateOverrides.map(([path, value]) => root.toggled
                ? `hecate msg set ${path} '${value}'`
                : `hecate msg unset ${path}`);
            Quickshell.execDetached(["bash", "-c", cmds.join("; ")])
            return
        }
        if (root.toggled) {
            Quickshell.execDetached(["bash", "-c", `hyprctl --batch "keyword animations:enabled 0; keyword decoration:shadow:enabled 0; keyword decoration:blur:enabled 0; keyword general:gaps_in 0; keyword general:gaps_out 0; keyword general:border_size 1; keyword decoration:rounding 0; keyword general:allow_tearing 1"`])
        } else {
            Quickshell.execDetached(["hyprctl", "reload"])
        }
    }
    Process {
        id: fetchActiveState
        running: true
        command: WM.compositor === "hecate"
            ? ["bash", "-c", `test "$(hecate msg -j get animations.enable)" = true`]
            : ["bash", "-c", `test "$(hyprctl getoption animations:enabled -j | jq ".int")" -ne 0`]
        onExited: (exitCode, exitStatus) => {
            root.toggled = exitCode !== 0 // Inverted because enabled = nonzero exit
        }
    }
    StyledToolTip {
        text: Translation.tr("Game mode")
    }
}