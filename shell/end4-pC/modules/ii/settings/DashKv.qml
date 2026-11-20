import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H

// A stat: the 12px uppercase label over its value (`.kv`).
ColumnLayout {
    id: root

    property string label: ""
    property string value: ""
    property string ink: "text"
    // 20px value (kv_value); off, the value is body text
    property bool large: true
    property string after: ""

    spacing: 0

    H.HLabel {
        text: root.label
    }
    Row {
        spacing: H.HTokens.ch()
        H.HText {
            text: root.value
            role: root.large ? "valueLg" : "body"
            ink: root.ink
        }
        H.HText {
            visible: root.after.length > 0
            anchors.baseline: parent.children[0].baseline
            text: root.after
            ink: "muted"
        }
    }
}
