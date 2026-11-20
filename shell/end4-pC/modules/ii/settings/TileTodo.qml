pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// todo and timer: the first tasks as chips (done ones checked, a click flips
// them) and the running countdown or pomodoro in gold.
DashTile {
    id: root

    readonly property var items: Todo.list ?? []
    readonly property int done: items.filter(i => i.done).length
    title: "todo"
    sub: items.length > 0 ? `${done} / ${items.length}` : ""

    readonly property string timer: {
        if (TimerService.countdownRunning) return TimerService.formatSeconds(TimerService.countdownSecondsLeft);
        if (TimerService.pomodoroRunning) return TimerService.formatSeconds(TimerService.pomodoroSecondsLeft);
        return "";
    }

    Repeater {
        model: root.items.slice(0, 5)
        H.Chip {
            required property int index
            required property var modelData
            Layout.maximumWidth: root.width - 2 * H.HTokens.space4
            text: modelData.content
            on: modelData.done
            interactive: true
            clip: true
            onClicked: modelData.done ? Todo.markUnfinished(index) : Todo.markDone(index)
        }
    }
    H.HText {
        visible: root.items.length === 0
        Layout.fillWidth: true
        text: "Nothing to do."
        role: "code"
        ink: "rose"
        wrapMode: Text.Wrap
    }

    Item { Layout.fillHeight: true }

    DashKv {
        label: "timer"
        value: root.timer || "off"
        ink: root.timer ? "gold" : "muted"
    }
}
