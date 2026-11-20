pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.services.claude
import qs.modules.common.widgets.haiiro as H

// claude: brief · a few plain sentences about the evening, written by Claude
// from the calendar, weather, repos and updates. It is written when you press
// `write`, or every hour once you turn `hourly` on (off until you do). `sent`
// shows the exact lines handed to Claude.
H.HWindow {
    id: root

    property bool live: true
    property bool showContext: false

    label: "claude: brief"
    widget: true
    implicitWidth: 420
    note: {
        if (ClaudeBrief.running) return "writing…";
        if (ClaudeBrief.error !== "") return ClaudeBrief.error;
        if (ClaudeBrief.writtenAt === 0) return "not written yet";
        const w = `written ${ClaudeBrief.stamp(ClaudeBrief.writtenAt)}`;
        return ClaudeBrief.scheduled ? `${w} · next at ${ClaudeBrief.stamp(ClaudeBrief.nextAt)}` : `${w} · hourly is off`;
    }

    // the repo list is only read while something shows it
    Component.onCompleted: Repos.consumers++
    Component.onDestruction: Repos.consumers--

    H.HText {
        Layout.fillWidth: true
        wrapMode: Text.WordWrap
        ink: ClaudeBrief.text !== "" ? "c4" : "c1"
        text: ClaudeBrief.text !== "" ? ClaudeBrief.text : "No brief yet. Press write and Claude will read what is below and tell you about it."
    }

    H.CodeBlock {
        Layout.fillWidth: true
        visible: root.showContext
        lines: ClaudeBrief.contextLines.map(l => "# " + l)
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        H.InkButton {
            ink: "c3"
            text: ClaudeBrief.running ? "writing…" : ClaudeBrief.text !== "" ? "write again" : "write"
            enabled: !ClaudeBrief.running
            onClicked: ClaudeBrief.generate()
        }
        H.Chip {
            text: "hourly"
            on: ClaudeBrief.scheduled
            interactive: true
            onClicked: ClaudeBrief.setScheduled(!ClaudeBrief.scheduled)
        }
        H.Chip {
            text: "sent"
            on: root.showContext
            interactive: true
            onClicked: root.showContext = !root.showContext
        }
    }
}
