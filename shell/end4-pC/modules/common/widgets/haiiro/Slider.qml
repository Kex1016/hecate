import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro

// A Progress track you can drag: 10px hairline track, text fill, and a 6x16
// square thumb ringed in surface. The label sits on the left in 8ch and the
// number on the right in 3ch. `thumb: false` is the OSD's read-only track.
// Arrow keys step by `stepSize`; Page Up/Down by ten steps.
Item {
    id: root

    property string label: ""
    property real from: 0
    property real to: 1
    property real value: 0
    property real stepSize: (to - from) / 20
    property bool thumb: true
    property bool showValue: true
    // what the number says; defaults to the position as a percentage
    property string valueText: Math.round(position * 100)
    readonly property real position: to === from ? 0 : Math.max(0, Math.min(1, (value - from) / (to - from)))
    readonly property bool pressed: drag.pressed
    signal moved(real value)

    activeFocusOnTab: thumb && enabled
    implicitWidth: 360
    implicitHeight: Math.max(18, HTokens.lineBox(HTokens.fontBody))
    opacity: enabled ? 1 : 0.55

    function setValue(v) {
        const s = root.stepSize > 0 ? Math.round((v - root.from) / root.stepSize) * root.stepSize + root.from : v;
        const c = Math.max(Math.min(root.from, root.to), Math.min(Math.max(root.from, root.to), s));
        if (c !== root.value) {
            root.value = c;
            root.moved(c);
        }
    }

    Keys.onPressed: event => {
        const k = event.key;
        if (k === Qt.Key_Right || k === Qt.Key_Up) root.setValue(root.value + root.stepSize);
        else if (k === Qt.Key_Left || k === Qt.Key_Down) root.setValue(root.value - root.stepSize);
        else if (k === Qt.Key_PageUp) root.setValue(root.value + 10 * root.stepSize);
        else if (k === Qt.Key_PageDown) root.setValue(root.value - 10 * root.stepSize);
        else if (k === Qt.Key_Home) root.setValue(root.from);
        else if (k === Qt.Key_End) root.setValue(root.to);
        else return;
        event.accepted = true;
    }

    RowLayout {
        anchors.fill: parent
        spacing: 12

        HText {
            visible: root.label.length > 0
            Layout.preferredWidth: 8 * HTokens.ch()
            text: root.label
            elide: Text.ElideRight
        }

        Item {
            id: track
            Layout.fillWidth: true
            Layout.preferredHeight: 10 + 2 * HTokens.borderHairline
            readonly property int inner: width - 2 * HTokens.borderHairline

            Rectangle {
                anchors.fill: parent
                color: HTokens.inkTrack
                border.width: HTokens.borderHairline
                border.color: HTokens.text
            }
            Rectangle {
                x: HTokens.borderHairline
                y: HTokens.borderHairline
                width: Math.round(track.inner * root.position)
                height: parent.height - 2 * HTokens.borderHairline
                color: HTokens.text
            }
            // thumb with its 2px surface ring
            Rectangle {
                visible: root.thumb
                x: HTokens.borderHairline + Math.round(track.inner * root.position) - 3 - 2
                y: HTokens.borderHairline - 4 - 2
                width: 10
                height: 20
                color: HTokens.surface

                Rectangle {
                    x: 2
                    y: 2
                    width: 6
                    height: 16
                    color: HTokens.text
                }
            }
            // focus: the same foam outline as buttons, around the track
            Rectangle {
                visible: root.activeFocus
                anchors.fill: parent
                anchors.margins: -8
                color: "transparent"
                border.width: 2
                border.color: HTokens.foam
            }
            MouseArea {
                id: drag
                enabled: root.thumb
                anchors.fill: parent
                anchors.margins: -6
                cursorShape: Qt.PointingHandCursor
                function apply(mx) {
                    const p = Math.max(0, Math.min(1, (mx - 6 - HTokens.borderHairline) / track.inner));
                    root.setValue(root.from + p * (root.to - root.from));
                }
                onPressed: mouse => { root.forceActiveFocus(); apply(mouse.x); }
                onPositionChanged: mouse => { if (pressed) apply(mouse.x); }
                onWheel: wheel => root.setValue(root.value + (wheel.angleDelta.y > 0 ? 1 : -1) * root.stepSize)
            }
        }

        HText {
            visible: root.showValue
            Layout.preferredWidth: 3 * HTokens.ch()
            text: root.valueText
        }
    }
}
