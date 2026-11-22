pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.services.claude
import qs.modules.common.widgets.haiiro as H

// claude: brief, free · a few sentences about the evening in rose at 16px, no wider than 400px,
// with when it was written under it. Claude writes it when you press `write`, or every hour once
// `hourly` is on (off until you do). `sent` prints the exact lines handed to Claude.
H.FreeWidget {
    id: root

    property bool showContext: false

    label: "claude: brief"
    implicitWidth: 400
    spacing: 8

    // the repo list is only read while something shows it
    Component.onCompleted: Repos.consumers++
    Component.onDestruction: Repos.consumers--

    H.FreeText {
        Layout.fillWidth: true
        size: 16
        wrapMode: Text.WordWrap
        ink: ClaudeBrief.text !== "" ? "rose" : "subtle"
        text: ClaudeBrief.text !== "" ? ClaudeBrief.text : "No brief yet. Press write and Claude will read what is sent and tell you about it."
    }

    H.FreeText {
        Layout.fillWidth: true
        role: "caption"
        ink: ClaudeBrief.error !== "" ? "love" : "muted"
        elide: Text.ElideRight
        text: {
            if (ClaudeBrief.running) return "writing…";
            if (ClaudeBrief.error !== "") return ClaudeBrief.error;
            if (ClaudeBrief.writtenAt === 0) return "not written yet";
            const w = `written ${ClaudeBrief.stamp(ClaudeBrief.writtenAt)}`;
            return ClaudeBrief.scheduled ? `${w} · next at ${ClaudeBrief.stamp(ClaudeBrief.nextAt)}` : `${w} · hourly is off`;
        }
    }

    H.CodeBlock {
        Layout.fillWidth: true
        visible: root.showContext
        lines: ClaudeBrief.contextLines.map(l => "# " + l)
    }

    Row {
        spacing: 4
        H.InkButton {
            flat: true
            role: "caption"
            text: ClaudeBrief.running ? "writing…" : ClaudeBrief.text !== "" ? "write again" : "write"
            enabled: !ClaudeBrief.running
            onClicked: ClaudeBrief.generate()
        }
        H.InkButton {
            flat: true
            role: "caption"
            text: "hourly"
            on: ClaudeBrief.scheduled
            onClicked: ClaudeBrief.setScheduled(!ClaudeBrief.scheduled)
        }
        H.InkButton {
            flat: true
            role: "caption"
            text: "sent"
            on: root.showContext
            onClicked: root.showContext = !root.showContext
        }
    }
}
