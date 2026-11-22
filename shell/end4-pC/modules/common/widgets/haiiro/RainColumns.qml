pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// Rain chance per hour: a dithered cloud line with one column per hour
// hanging from it, more foam drops for a higher chance (seven at 100%), the
// hour under each column in subtle.
//   RainColumns { hours: [{ hour: "22", chance: 80 }, { hour: "23", chance: 70 }] }
PixelGrid {
    id: root

    // [{ hour: "22", chance: 0-100 }]
    property var hours: []
    readonly property int maxDrops: 7
    readonly property real colW: px(26)
    readonly property real colGap: px(10)
    readonly property real colH: px(92)
    readonly property real cloudH: px(6)

    implicitWidth: Math.max(0, hours.length * (colW + colGap) - colGap)
    implicitHeight: cloudH + colH

    function drops(chance) {
        const c = Math.max(0, Math.min(100, Number(chance) || 0));
        return c <= 0 ? 0 : Math.max(1, Math.round(c * root.maxDrops / 100));
    }

    PixelChecker {
        width: root.width
        height: root.cloudH
        ink: "subtle"
    }

    Repeater {
        model: root.hours
        Item {
            id: col
            required property int index
            required property var modelData
            x: index * (root.colW + root.colGap)
            y: root.cloudH
            width: root.colW
            height: root.colH

            Repeater {
                model: root.drops(col.modelData.chance)
                Rectangle {
                    required property int index
                    x: index % 2 === 1 ? root.px(6) : 0
                    y: root.px(4) + index * (root.px(6) + root.px(4))
                    width: root.px(2)
                    height: root.px(6)
                    color: HTokens.foam
                }
            }
            FreeText {
                anchors.bottom: parent.bottom
                text: col.modelData.hour ?? ""
                role: "caption"
                ink: "subtle"
            }
        }
    }
}
