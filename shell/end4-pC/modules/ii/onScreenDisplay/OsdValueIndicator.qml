import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts

// One OSD window, 360px: the name in the notch and either a read-only track
// with its number, or (with `message`) a single centred line. `variant` is the
// frame colour: love for muted or refused, pine otherwise.
H.HWindow {
    id: root
    required property string name
    property real value: 0
    property real from: 0
    property real to: 1
    property string message: ""
    property string messageInk: "c2"

    width: H.HTokens.osdWidth
    label: name

    H.Slider {
        visible: root.message.length === 0
        Layout.fillWidth: true
        thumb: false
        from: root.from
        to: root.to
        value: root.value
    }

    H.HText {
        id: msg
        visible: root.message.length > 0
        Layout.fillWidth: true
        horizontalAlignment: Text.AlignHCenter
        text: root.message
        ink: root.messageInk
    }
}
