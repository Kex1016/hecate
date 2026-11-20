import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts

// One slider row: the name (and what it drives, in muted) over the track,
// the number on the right. Clicking the name opens `list` in place.
ColumnLayout {
    id: root

    property string label: ""
    property string detail: ""
    property string list: ""
    property alias from: slider.from
    property alias to: slider.to
    property alias value: slider.value
    property alias stepSize: slider.stepSize
    property alias valueText: slider.valueText
    signal moved(real value)
    signal listRequested()

    spacing: H.HTokens.space1

    Item {
        Layout.fillWidth: true
        implicitHeight: head.implicitHeight

        RowLayout {
            id: head
            anchors.fill: parent
            spacing: H.HTokens.ch()

            H.HText {
                text: root.label
                role: "code"
            }
            H.HText {
                Layout.fillWidth: true
                text: root.detail
                role: "caption"
                ink: "c1"
                elide: Text.ElideRight
            }
            H.HText {
                id: number
                text: root.enabled && isFinite(slider.position) ? slider.valueText : "--"
                role: "code"
                ink: "inkMuted"
            }
        }

        // the name and what it drives open the list; the number does nothing
        MouseArea {
            enabled: root.list !== ""
            anchors {
                left: parent.left
                top: parent.top
                bottom: parent.bottom
            }
            width: number.x
            cursorShape: Qt.PointingHandCursor
            onClicked: root.listRequested()
        }
    }

    H.Slider {
        id: slider
        Layout.fillWidth: true
        showValue: false
        onMoved: v => root.moved(v)
    }
}
