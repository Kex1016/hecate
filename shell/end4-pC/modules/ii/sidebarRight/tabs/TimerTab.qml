import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts

// Pomodoro, stopwatch and countdown from TimerService, which keeps running
// while the panel is closed.
ColumnLayout {
    id: root

    property int mode: 0

    function pad(n) {
        return n.toString().padStart(2, "0");
    }
    // stopwatch time is in 10ms
    function stopwatchText(t) {
        const cs = t % 100, s = Math.floor(t / 100);
        return pad(Math.floor(s / 60)) + ":" + pad(s % 60) + "." + pad(cs);
    }

    readonly property string phase: TimerService.pomodoroLongBreak ? "long break" : TimerService.pomodoroBreak ? "break" : "focus"

    spacing: H.HTokens.space3

    H.Segmented {
        model: ["pomodoro", "stopwatch", "countdown"]
        currentIndex: root.mode
        onActivated: i => root.mode = i
    }

    H.HText {
        Layout.fillWidth: true
        role: "nav"
        text: root.mode === 0 ? TimerService.formatSeconds(TimerService.pomodoroSecondsLeft)
            : root.mode === 1 ? root.stopwatchText(TimerService.stopwatchTime)
            : TimerService.formatSeconds(TimerService.countdownSecondsLeft)
        ink: (root.mode === 0 && TimerService.pomodoroRunning) || (root.mode === 1 && TimerService.stopwatchRunning) || (root.mode === 2 && TimerService.countdownRunning) ? "text" : "subtle"
    }

    H.HText {
        Layout.fillWidth: true
        role: "caption"
        ink: "c1"
        text: root.mode === 0 ? root.phase + " · cycle " + (TimerService.pomodoroCycle + 1) + " of " + TimerService.cyclesBeforeLongBreak
            : root.mode === 1 ? (TimerService.stopwatchLaps.length > 0 ? TimerService.stopwatchLaps.length + " laps, last " + root.stopwatchText(TimerService.stopwatchLaps[TimerService.stopwatchLaps.length - 1]) : "no laps")
            : "add minutes, then start"
    }

    Item {
        Layout.fillHeight: true
    }

    RowLayout {
        spacing: H.HTokens.space3

        H.InkButton {
            ink: "c3"
            readonly property bool running: root.mode === 0 ? TimerService.pomodoroRunning : root.mode === 1 ? TimerService.stopwatchRunning : TimerService.countdownRunning
            text: running ? "pause" : "start"
            onClicked: root.mode === 0 ? TimerService.togglePomodoro() : root.mode === 1 ? TimerService.toggleStopwatch() : TimerService.toggleCountdown()
        }
        H.InkButton {
            visible: root.mode === 1
            ink: "c5"
            text: "lap"
            enabled: TimerService.stopwatchRunning
            onClicked: TimerService.stopwatchRecordLap()
        }
        H.InkButton {
            visible: root.mode === 2
            ink: "c5"
            text: "+1m"
            onClicked: TimerService.addCountdownMinutes(1)
        }
        H.InkButton {
            visible: root.mode === 2
            ink: "c5"
            text: "+5m"
            onClicked: TimerService.addCountdownMinutes(5)
        }
        H.InkButton {
            ink: "c1"
            text: "reset"
            onClicked: root.mode === 0 ? TimerService.resetPomodoro() : root.mode === 1 ? TimerService.stopwatchReset() : TimerService.resetCountdown()
        }
    }
}
