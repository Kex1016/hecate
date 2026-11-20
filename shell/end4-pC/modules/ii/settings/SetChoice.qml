import QtQuick
import qs.modules.common.widgets.haiiro as H

// A choice of two to four words, the current one on a pine band.
//   SetChoice {
//       name: "clock"
//       key: "time.format"
//       values: ["hh:mm", "h:mm ap"]
//       labels: ["24h", "12h"]
//   }
SetRow {
    id: root

    property var values: []
    property var labels: values
    readonly property int index: Math.max(0, root.values.findIndex(v => JSON.stringify(v) === JSON.stringify(root.current)))

    H.Segmented {
        model: root.labels
        currentIndex: root.index
        onActivated: i => root.write(root.values[i])
    }
}
