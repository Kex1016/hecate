import QtQuick
import qs.modules.common.widgets.haiiro

// InkLink as a real button: a block of ink with surface text, 1ch padding,
// no border, no radius. `ink` picks the colour and the job:
//   c3 gold   the one button that acts (allow, apply, run), at most one per window
//   c2 love   refuses or deletes
//   c1 muted  dismisses
//   c5 pine   a lasting choice ("always in this repo")
//   ""        text-coloured block
// `flat` is a quiet text button; `on` lifts a flat button to text colour.
// Hover and press invert it, like the site's inverse links. Focus is a 2px
// foam outline, 2px out.
Item {
    id: root

    property string text: ""
    property string ink: ""
    property bool flat: false
    property bool on: false
    property string role: "body"
    readonly property bool hovered: mouse.containsMouse
    readonly property bool pressed: mouse.pressed
    signal clicked()

    readonly property color fill: ink === "" ? HTokens.text : HTokens.ink(ink)
    // pine and muted carry text; the light inks carry surface
    readonly property color label: (ink === "c1" || ink === "c5" || ink === "muted" || ink === "pine") ? HTokens.text : HTokens.surface
    readonly property bool inverted: !flat && (hovered || pressed)

    activeFocusOnTab: enabled
    implicitWidth: caption.implicitWidth + 2 * HTokens.ch(caption.typeStyle.size)
    implicitHeight: caption.implicitHeight
    opacity: enabled ? 1 : 0.55

    Keys.onPressed: event => {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.clicked();
            event.accepted = true;
        }
    }

    Rectangle {
        anchors.fill: parent
        color: root.flat || root.inverted ? "transparent" : root.fill
    }

    HText {
        id: caption
        anchors.centerIn: parent
        text: root.text
        role: root.role
        color: root.flat ? (root.on || root.hovered ? HTokens.text : HTokens.subtle) : root.inverted ? root.fill : root.label
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
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
