import QtQuick
import Quickshell.Hyprland
import qs.services

Loader {
    id: root

    property string name: ""
    property string description: ""
    signal pressed()
    signal released()

    active: WM.compositor === "hyprland"

    // Elsewhere (hecate) the compositor's binds fire us through ShortcutRegistry's IPC.
    Component.onCompleted: if (WM.compositor === "hecate") ShortcutRegistry.register(root)
    Component.onDestruction: if (WM.compositor === "hecate") ShortcutRegistry.unregister(root)

    sourceComponent: GlobalShortcut {
        name: root.name
        description: root.description
        onPressed: root.pressed()
        onReleased: root.released()
    }
}
