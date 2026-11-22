import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// `timer: focus`: the pomodoro, driven by the same TimerService as the quick
// panel and the window, so all of them always agree. A ring of 60 squares
// around the time; the squares used up go dark, one for every 60th of the
// round (25 seconds of a 25 minute focus).
H.FreeWidget {
    id: root

    readonly property string mode: TimerService.pomodoroLongBreak ? "long" : TimerService.pomodoroBreak ? "break" : "focus"
    readonly property int total: Math.max(1, TimerService.pomodoroLapDuration)
    readonly property int remaining: Math.max(0, TimerService.pomodoroSecondsLeft)

    label: "timer: " + root.mode
    spacing: 10

    H.PixelRing {
        done: Math.max(0, Math.min(60, Math.floor((1 - root.remaining / root.total) * 60)))
        ink: root.mode === "focus" ? "gold" : "foam"

        ColumnLayout {
            anchors.centerIn: parent
            spacing: 4
            H.FreeText {
                Layout.alignment: Qt.AlignHCenter
                role: "nav"
                ink: root.mode === "focus" ? "gold" : "foam"
                text: TimerService.formatSeconds(root.remaining)
            }
            H.FreeText {
                Layout.alignment: Qt.AlignHCenter
                role: "caption"
                ink: "subtle"
                text: root.mode + " · " + (TimerService.pomodoroCycle + 1) + " of " + TimerService.cyclesBeforeLongBreak
            }
        }
    }
    RowLayout {
        spacing: 8
        H.InkButton {
            ink: "c3"
            text: TimerService.pomodoroRunning ? "pause" : "start"
            onClicked: TimerService.togglePomodoro()
        }
        H.InkButton {
            ink: "c1"
            text: "reset"
            onClicked: TimerService.resetPomodoro()
        }
    }
}
