import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// `timer: focus`: the pomodoro, driven by the same TimerService as the quick
// panel, so both always agree.
H.HWindow {
    id: root

    readonly property string mode: TimerService.pomodoroLongBreak ? "long" : TimerService.pomodoroBreak ? "break" : "focus"
    readonly property int total: Math.max(1, TimerService.pomodoroLapDuration)
    readonly property int remaining: Math.max(0, TimerService.pomodoroSecondsLeft)

    widget: true
    label: "timer: " + root.mode
    implicitWidth: 380

    H.HText {
        role: "nav"
        ink: "c3"
        text: TimerService.formatSeconds(root.remaining)
    }
    H.Progress {
        Layout.fillWidth: true
        value: 1 - root.remaining / root.total
    }
    RowLayout {
        spacing: 6
        H.Chip { text: "focus"; on: root.mode === "focus" }
        H.Chip { text: "break"; on: root.mode === "break" }
        H.Chip { text: "long"; on: root.mode === "long" }
    }
    H.HText {
        role: "caption"
        ink: "c1"
        text: "round " + (TimerService.pomodoroCycle + 1) + " of " + TimerService.cyclesBeforeLongBreak
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
