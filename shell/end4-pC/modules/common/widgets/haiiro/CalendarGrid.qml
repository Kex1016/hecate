pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// A month as a seven-column grid, Monday first: weekday headings in 12px
// ink-muted, the previous month's tail muted, today on a pine band, days
// with events in gold.
//   CalendarGrid { year: 2026; month: 10; eventDays: [21] }   // month is 0-11
Item {
    id: root

    property int year: new Date().getFullYear()
    property int month: new Date().getMonth()
    property date today: new Date()
    // day numbers of this month that have events
    property var eventDays: []
    property var weekdays: ["MO", "TU", "WE", "TH", "FR", "SA", "SU"]
    signal dayClicked(date day)

    readonly property int lead: (new Date(year, month, 1).getDay() + 6) % 7
    readonly property int days: new Date(year, month + 1, 0).getDate()
    readonly property int prevDays: new Date(year, month, 0).getDate()
    readonly property real colW: width / 7
    readonly property int cellH: HTokens.lineBox(HTokens.fontBody) + 6
    readonly property int headH: HTokens.lineBox(HTokens.fontLabel) + 6

    readonly property var cells: {
        const out = [];
        for (let i = 0; i < root.lead; i++)
            out.push({ day: root.prevDays - root.lead + 1 + i, other: true });
        for (let d = 1; d <= root.days; d++)
            out.push({ day: d, other: false });
        return out;
    }

    implicitWidth: 300
    implicitHeight: headH + 2 + Math.ceil(cells.length / 7) * (cellH + 2) - 2

    Repeater {
        model: root.weekdays
        HText {
            required property int index
            required property string modelData
            x: Math.round(index * root.colW)
            width: Math.round((index + 1) * root.colW) - x
            height: root.headH
            horizontalAlignment: Text.AlignHCenter
            text: modelData
            role: "caption"
            ink: "inkMuted"
            font.letterSpacing: HTokens.trackLabel
        }
    }

    Repeater {
        model: root.cells
        Rectangle {
            id: day
            required property int index
            required property var modelData
            readonly property int col: index % 7
            readonly property int rowN: Math.floor(index / 7)
            readonly property bool isToday: !modelData.other && root.today.getFullYear() === root.year && root.today.getMonth() === root.month && root.today.getDate() === modelData.day
            readonly property bool hasEvent: !modelData.other && root.eventDays.indexOf(modelData.day) >= 0

            x: Math.round(col * root.colW)
            y: root.headH + 2 + rowN * (root.cellH + 2)
            width: Math.round((col + 1) * root.colW) - x
            height: root.cellH
            color: isToday ? HTokens.pine : "transparent"

            HText {
                anchors.fill: parent
                horizontalAlignment: Text.AlignHCenter
                text: day.modelData.day
                ink: day.modelData.other ? "muted" : day.isToday ? "text" : day.hasEvent ? "gold" : "text"
            }
            MouseArea {
                anchors.fill: parent
                enabled: !day.modelData.other
                cursorShape: Qt.PointingHandCursor
                onClicked: root.dayClicked(new Date(root.year, root.month, day.modelData.day))
            }
        }
    }
}
