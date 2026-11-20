pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import qs.modules.ii.sidebarRight
import QtQuick
import QtQuick.Layouts

// The bottom of the panel: [calendar] todo timer, the current one in pine
// brackets. Every tab gets the same height (six calendar weeks), so the
// notifications above never jump.
ColumnLayout {
    id: root

    readonly property var tabs: ["calendar", "todo", "timer"]
    readonly property int bodyHeight: cal.headH + 2 + 6 * (cal.cellH + 2) - 2

    spacing: H.HTokens.space2

    H.HRule {
        Layout.fillWidth: true
        Layout.topMargin: -6
        Layout.bottomMargin: -6
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: H.HTokens.space6

        Repeater {
            model: root.tabs
            H.HText {
                id: tab
                required property string modelData
                readonly property bool current: QuickState.tab === modelData
                text: current ? "[" + modelData + "]" : modelData
                ink: current ? "c5" : mouse.containsMouse ? "text" : "c1"

                MouseArea {
                    id: mouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: QuickState.tab = tab.modelData
                }
            }
        }

        Item {
            Layout.fillWidth: true
        }

        // the calendar's month, and the way to the others
        Row {
            visible: QuickState.tab === "calendar"
            spacing: 0
            H.InkButton {
                flat: true
                text: "‹"
                onClicked: cal.shift(-1)
            }
            H.HText {
                text: Qt.locale("en_GB").monthName(cal.month, Locale.LongFormat).toLowerCase() + " " + cal.year
                role: "caption"
                ink: cal.isThisMonth ? "c1" : "text"
                anchors.verticalCenter: parent.verticalCenter

                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: cal.reset()
                }
            }
            H.InkButton {
                flat: true
                text: "›"
                onClicked: cal.shift(1)
            }
        }
        H.HText {
            visible: QuickState.tab === "todo"
            text: Todo.list.filter(t => !t.done).length + " left"
            role: "caption"
            ink: "c1"
        }
    }

    Item {
        Layout.fillWidth: true
        Layout.preferredHeight: root.bodyHeight

        H.CalendarGrid {
            id: cal
            visible: QuickState.tab === "calendar"
            width: parent.width
            today: DateTime.clock.date
            // TODO(W2-icloud): days with events, from the calendars service
            eventDays: []
            readonly property bool isThisMonth: year === today.getFullYear() && month === today.getMonth()

            function shift(n) {
                const d = new Date(year, month + n, 1);
                year = d.getFullYear();
                month = d.getMonth();
            }
            function reset() {
                year = today.getFullYear();
                month = today.getMonth();
            }

            Component.onCompleted: reset()

            WheelHandler {
                onWheel: event => cal.shift(event.angleDelta.y > 0 ? -1 : 1)
            }
        }

        Loader {
            anchors.fill: parent
            active: QuickState.tab === "todo"
            sourceComponent: TodoTab {}
        }

        Loader {
            anchors.fill: parent
            active: QuickState.tab === "timer"
            sourceComponent: TimerTab {}
        }
    }
}
