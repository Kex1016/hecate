pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// claude /usage, free · the share of the five-hour window in Jacquard, twenty foam squares of
// 5% each, and a gold "N waiting" on the last line when a session needs you. Reads Claude
// Code's own logs through the ClaudeUsage service; nothing is sent anywhere.
H.FreeWidget {
    id: root

    label: "claude /usage"
    implicitWidth: 238
    spacing: 8

    RowLayout {
        Layout.fillWidth: true
        spacing: 14
        H.FreeText {
            role: "display"
            size: 72
            text: `${ClaudeUsage.percent}%`
            ink: ClaudeUsage.percentage >= 0.9 ? "love" : ClaudeUsage.percentage >= 0.7 ? "gold" : "text"
        }
        H.FreeText {
            Layout.alignment: Qt.AlignBottom
            Layout.bottomMargin: 8
            role: "caption"
            ink: "subtle"
            text: "of this window"
        }
    }

    H.PixelCells {
        large: true
        count: 20
        filled: Math.round(ClaudeUsage.percentage * 20)
        ink: "foam"
    }

    Row {
        spacing: 0
        H.FreeText {
            role: "caption"
            ink: "muted"
            text: {
                const n = ClaudeUsage.activeSessions;
                return `${ClaudeUsage.resetText !== "" ? `resets ${ClaudeUsage.resetText}` : "no window open"} · ${n} session${n === 1 ? "" : "s"} today`;
            }
        }
        H.FreeText {
            visible: ClaudeUsage.waitingSessions > 0
            role: "caption"
            ink: "gold"
            text: ` · ${ClaudeUsage.waitingSessions} waiting`
        }
    }
}
