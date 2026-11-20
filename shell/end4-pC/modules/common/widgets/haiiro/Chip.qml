import QtQuick
import qs.modules.common.widgets.haiiro

// A small square tag. Off: dashed ink-muted frame and text. `on`: solid
// text frame, a check mark in front. On/off settings use a chip.
// `badge`: the solid uppercase 11px tag that sits after a heading (`NEW`).
Item {
    id: root

    property string text: ""
    property bool on: false
    property bool badge: false
    // a chip that toggles: clicked() fires, the owner flips `on`
    property bool interactive: false
    readonly property bool hovered: mouse.containsMouse
    signal clicked()

    activeFocusOnTab: interactive && enabled
    implicitWidth: caption.implicitWidth + 12 + 2 * HTokens.borderHairline
    implicitHeight: badge ? caption.implicitHeight + 2 * HTokens.borderHairline : Math.round(HTokens.fontLabel * 1.6) + 2 * HTokens.borderHairline
    opacity: enabled ? 1 : 0.55

    Keys.onPressed: event => {
        if (interactive && (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) {
            root.clicked();
            event.accepted = true;
        }
    }

    HDashFrame {
        anchors.fill: parent
        dashed: !root.on && !root.badge
        color: root.on || root.badge ? HTokens.text : HTokens.inkMuted
    }

    HText {
        id: caption
        x: 6 + HTokens.borderHairline
        anchors.verticalCenter: parent.verticalCenter
        text: (root.on && !root.badge ? "✓ " : "") + root.text
        role: root.badge ? "badge" : "caption"
        color: root.on || root.badge || root.hovered ? HTokens.text : HTokens.inkMuted
    }

    Rectangle {
        visible: root.activeFocus
        anchors.fill: parent
        anchors.margins: -4
        color: "transparent"
        border.width: 2
        border.color: HTokens.foam
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        enabled: root.interactive
        hoverEnabled: root.interactive
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
