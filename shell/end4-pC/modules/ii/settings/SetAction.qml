pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro as H

// A row that does something: one or more ink buttons.
//   SetAction {
//       name: "check for updates"
//       buttons: [{ text: "check", ink: "c3" }]
//       onPressed: i => Updates.refresh()
//   }
SetRow {
    id: root

    property var buttons: []
    signal pressed(int index)

    Row {
        spacing: 8
        Repeater {
            model: root.buttons
            H.InkButton {
                required property int index
                required property var modelData
                text: modelData.text
                ink: modelData.ink ?? "c1"
                onClicked: root.pressed(index)
            }
        }
    }
}
