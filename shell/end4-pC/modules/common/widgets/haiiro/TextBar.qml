import QtQuick
import qs.modules.common.widgets.haiiro

// htop's own bar, in text: `[||||||··················]`. The filled part is
// `ink` (foam), the rest dots in highlight-high, the brackets in text.
// `cells` is how many characters the bar holds.
//   Row { spacing: 8
//       FreeText { text: "cpu"; ink: "subtle" }
//       TextBar { value: 0.23 }
//       FreeText { text: "23%"; ink: "foam" } }
Row {
    id: root

    // 0-1
    property real value: 0
    property int cells: 24
    property string ink: "foam"
    property string role: "body"

    readonly property int filled: Math.max(0, Math.min(cells, Math.round(value * cells)))

    spacing: 0

    FreeText { text: "["; role: root.role }
    FreeText { text: "|".repeat(root.filled); role: root.role; ink: root.ink; visible: root.filled > 0 }
    FreeText { text: "·".repeat(root.cells - root.filled); role: root.role; ink: "highlightHigh"; visible: root.filled < root.cells }
    FreeText { text: "]"; role: root.role }
}
