import QtQuick
import qs.modules.common.widgets.haiiro as H

// An on/off setting: a chip that reads `on`/`off` (or `onText`/`offText`).
//   SetToggle {
//       name: "narration line"
//       key: "background.clock.narration"
//   }
SetRow {
    id: root

    property string onText: "on"
    property string offText: "off"
    // for a key whose "on" is the opposite of its value
    property bool inverted: false
    readonly property bool checked: (root.current === true) !== root.inverted

    function toggle() {
        root.write(!root.checked !== root.inverted);
    }

    H.Chip {
        id: chip
        interactive: true
        on: root.checked
        text: root.checked ? root.onText : root.offText
        onClicked: root.toggle()
    }
}
