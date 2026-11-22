pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// The day as 24 hour ticks: the hours gone in text, the current one taller in
// pine, the hours to come in highlight-med; every sixth tick is taller. With
// `labels` the hours 00 06 12 18 sit under it in subtle, as one spaced line 6px below.
//   DayRuler { hour: 21 }
PixelGrid {
    id: root

    // 0-23
    property int hour: 0
    property bool labels: true

    readonly property real tick: px(HTokens.pixelCell)
    readonly property real pitch: tick + px(3)
    readonly property real rulerH: px(16)

    implicitWidth: 24 * pitch - px(3)
    implicitHeight: rulerH + (labels ? 6 + caption.implicitHeight : 0)

    Repeater {
        model: 24
        Rectangle {
            required property int index
            readonly property bool now: index === root.hour
            x: index * root.pitch
            width: root.tick
            height: now ? root.rulerH : index % 6 === 0 ? root.px(14) : root.tick
            y: root.rulerH - height
            color: now ? HTokens.pine : index < root.hour ? HTokens.text : HTokens.highlightMed
        }
    }

    FreeText {
        id: caption
        visible: root.labels
        y: root.rulerH + 6
        text: "00    06    12    18   "
        role: "code"
        ink: "subtle"
        lineHeight: Math.round(typeStyle.size * 1.25)
    }
}
