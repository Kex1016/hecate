pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// claude: usage · how much of the current five-hour window is used (from the
// ClaudeUsage service, which reads Claude Code's own logs), and when it resets.
// Reads local files only; nothing is sent anywhere.
H.HWindow {
    id: root

    property bool live: true

    label: "claude: usage"
    widget: true
    implicitWidth: 400

    RowLayout {
        Layout.fillWidth: true
        spacing: H.HTokens.ch(H.HTokens.fontBody)
        H.HText {
            role: "valueLg"
            text: `${ClaudeUsage.percent}%`
            ink: ClaudeUsage.percentage >= 0.9 ? "c2" : ClaudeUsage.percentage >= 0.7 ? "c3" : "text"
        }
        H.HText {
            Layout.fillWidth: true
            ink: "c1"
            text: "of this window"
        }
    }

    H.Progress {
        Layout.fillWidth: true
        value: ClaudeUsage.percentage
    }

    H.HText {
        Layout.fillWidth: true
        role: "caption"
        ink: "c1"
        elide: Text.ElideRight
        text: {
            const parts = [];
            parts.push(ClaudeUsage.resetText !== "" ? `resets ${ClaudeUsage.resetText}` : "no window open");
            const n = ClaudeUsage.activeSessions;
            parts.push(`${n} session${n === 1 ? "" : "s"}`);
            if (ClaudeUsage.waitingSessions > 0) parts.push(`${ClaudeUsage.waitingSessions} waiting`);
            return parts.join(" · ");
        }
    }
}
