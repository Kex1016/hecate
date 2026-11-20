pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// Charts made of squares, never lines. Heights are the data; there are no
// axes, so put the current value as text beside the chart.
//   Spark { values: [3, 5, 2, 8] }                       // 6px columns, 2px apart, 40px tall
//   Spark { values: [...]; ink: "iris" }
//   Spark { values: [...]; labels: ["22", "23", "00"] }  // the weather's hourly bars, 56px tall
Item {
    id: root

    property var values: []
    property var labels: []
    property string ink: "foam"
    // the value that fills the whole height; defaults to the largest value
    property real max: 0
    readonly property bool hourly: labels.length > 0
    readonly property real peak: max > 0 ? max : Math.max(1, ...values)
    readonly property int labelH: HTokens.lineBox(HTokens.fontBadge)
    readonly property int chartH: hourly ? height - labelH - 2 : height

    implicitHeight: hourly ? 56 : 40
    implicitWidth: hourly ? 320 : Math.max(0, values.length * 8 - 2)

    function barHeight(v) {
        return Math.max(1, Math.round(Math.max(0, v) / root.peak * root.chartH));
    }

    // sparkline: fixed 6px columns
    Row {
        visible: !root.hourly
        anchors.bottom: parent.bottom
        spacing: 2
        Repeater {
            model: root.hourly ? [] : root.values
            Rectangle {
                required property real modelData
                anchors.bottom: parent.bottom
                width: 6
                height: root.barHeight(modelData)
                color: HTokens.ink(root.ink)
            }
        }
    }

    // hourly: equal columns with their label under the bar
    Repeater {
        model: root.hourly ? root.values : []
        Item {
            id: col
            required property int index
            required property real modelData
            readonly property real colW: (root.width - 4 * (root.values.length - 1)) / root.values.length
            x: Math.round(index * (colW + 4))
            width: Math.round(colW)
            height: root.height

            Rectangle {
                y: root.chartH - height
                width: parent.width
                height: root.barHeight(col.modelData)
                color: HTokens.ink(root.ink)
            }
            HText {
                y: root.chartH + 2
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: root.labels[col.index] ?? ""
                role: "badge"
                font.capitalization: Font.MixedCase
                font.letterSpacing: 0
                ink: "inkMuted"
            }
        }
    }
}
