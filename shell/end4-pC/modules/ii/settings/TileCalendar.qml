import QtQuick
import QtQuick.Layouts
import qs.services
import qs.services.apple
import qs.modules.common.widgets.haiiro as H

// the month: this month's grid, days with events in gold, and the next event
// with the calendar it came from.
DashTile {
    id: root

    readonly property date now: DateTime.clock.date
    title: Qt.locale("en_GB").monthName(now.getMonth(), Locale.LongFormat).toLowerCase()
    interactive: true

    readonly property var monthEvents: CalendarAccounts.eventsIn(now.getFullYear(), now.getMonth())
    readonly property var next: CalendarAccounts.nextEvent(now)

    H.CalendarGrid {
        Layout.fillWidth: true
        year: root.now.getFullYear()
        month: root.now.getMonth()
        today: root.now
        // every day an event touches, multi-day ones included
        eventDays: Calendars.eventDays(root.now.getFullYear(), root.now.getMonth())
        onDayClicked: root.clicked()
    }

    Row {
        Layout.fillWidth: true
        Layout.topMargin: 0
        spacing: H.HTokens.ch()
        visible: root.next !== null
        H.HText {
            text: root.next ? Qt.locale("en_GB").toString(new Date(root.next.start), "ddd d").toLowerCase() : ""
            role: "code"
            ink: "gold"
        }
        H.HText {
            text: root.next?.title ?? ""
            role: "code"
        }
        H.HText {
            anchors.verticalCenter: parent.verticalCenter
            text: root.next ? [root.next.account, root.next.calendar].filter(s => s).join(" · ") : ""
            role: "caption"
            ink: "muted"
        }
    }
    H.HText {
        visible: root.next === null
        Layout.fillWidth: true
        text: CalendarAccounts.accounts.length === 0 ? "No calendars yet. Add one under calendars." : "Nothing ahead. The days are yours."
        role: "code"
        ink: "rose"
        wrapMode: Text.Wrap
    }
    Item { Layout.fillHeight: true }
}
