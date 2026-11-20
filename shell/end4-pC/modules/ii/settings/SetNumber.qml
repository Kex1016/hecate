import QtQuick
import qs.modules.common.widgets.haiiro as H

// A number, stepped with ← → or a click on an arrow: `‹ 5s ›`. `divisor`
// divides the stored value for display (milliseconds shown as seconds:
// divisor 1000).
//   SetNumber {
//       name: "toast timeout"
//       key: "notifications.timeout"
//       from: 1
//       to: 30
//       divisor: 1000
//       suffix: "s"
//   }
SetRow {
    id: root

    property real from: 0
    property real to: 100
    property real stepSize: 1
    property real divisor: 1
    property string suffix: ""
    // shown instead of the number at `from` ("never")
    property string zeroText: ""
    readonly property real shown: (Number(root.current) || 0) / root.divisor

    onShownChanged: spin.value = root.shown

    H.Spin {
        id: spin
        value: root.shown
        from: root.from
        to: root.to
        stepSize: root.stepSize
        suffix: root.suffix
        text: root.zeroText !== "" && value === root.from ? root.zeroText : Math.round(value * 100) / 100 + root.suffix
        onMoved: v => root.write(Math.round(v * root.divisor * 1000) / 1000)
        // ↑ ↓ move between rows; ← → change the number.
        Keys.onUpPressed: event => {
            root.navigate(-1);
            event.accepted = true;
        }
        Keys.onDownPressed: event => {
            root.navigate(1);
            event.accepted = true;
        }
    }
}
