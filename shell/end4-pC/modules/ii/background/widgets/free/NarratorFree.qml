import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro as H
import "Narration.js" as Narration

// `fortune`: one line about now, picked from what is true: how long the
// machine has been up, the weather, the battery, the calendar, the hour.
// The line only changes when the hour does, so it never flickers. The one
// widget allowed to be large rose type, with a cursor at the end of it.
H.FreeWidget {
    id: root

    SystemClock {
        id: clock
        enabled: root.live
        precision: SystemClock.Minutes
    }

    readonly property string line: Narration.line(clock.date, {
        uptime: DateTime.uptime,
        weather: Weather.data?.description,
        battery: Battery,
        nextEvent: DesktopWidgets.nextEvent
    })

    label: "fortune"

    Item {
        id: body
        Layout.preferredWidth: Math.min(520, quote.implicitWidth)
        implicitHeight: quote.implicitHeight

        H.FreeText {
            id: quote
            width: parent.width
            role: "nav"
            ink: "rose"
            wrapMode: Text.WordWrap
            text: root.line
        }
        // the same text, unseen, for where its last character ends
        TextEdit {
            id: probe
            width: quote.width
            height: quote.height
            opacity: 0
            enabled: false
            readOnly: true
            wrapMode: TextEdit.WordWrap
            text: quote.text
            font: quote.font
        }
        // 8x16 after the last character, 8px away, sitting on the baseline
        H.BlockCursor {
            readonly property rect end: probe.positionToRectangle(probe.length)
            ink: "rose"
            x: end.x + 8
            y: end.y + quote.lineHeight - implicitHeight - 7
        }
    }
}
