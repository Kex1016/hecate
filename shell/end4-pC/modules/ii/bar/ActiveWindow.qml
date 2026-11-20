pragma ComponentBehavior: Bound
import QtQuick
import Quickshell.Wayland
import qs.modules.common.widgets.haiiro

// The focused window after a muted », named the way its frame is
// ("kitty: ~/src/hecate"). It gives way first when the bar runs short.
Item {
    id: root

    readonly property Toplevel window: ToplevelManager.activeToplevel
    readonly property string label: root.window ? BarState.windowLabel(root.window.appId, root.window.title) : ""

    // stays in the layout when empty, so the segments before it keep their place
    implicitWidth: mark.implicitWidth + HTokens.space4 + title.implicitWidth
    implicitHeight: title.implicitHeight

    HText {
        id: mark
        anchors.verticalCenter: parent.verticalCenter
        visible: root.label.length > 0
        text: "»"
        ink: "muted"
    }

    HText {
        id: title
        anchors.verticalCenter: parent.verticalCenter
        x: mark.width + HTokens.space4
        width: Math.max(0, root.width - x)
        text: root.label
        ink: "subtle"
        elide: Text.ElideRight
    }
}
