pragma Singleton
import qs.modules.common
import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root

    property var sessions: []
    property int tokens: 0
    property int resetsAt: 0
    readonly property int tokenLimit: Config.options.bar.aiUsage.tokenLimit
    readonly property real percentage: root.tokenLimit > 0 ? Math.min(root.tokens / root.tokenLimit, 1) : 0
    readonly property int activeSessions: root.sessions.length
    // sessions Claude Code reports as asking for something ("waiting", "needs input", ...)
    readonly property int waitingSessions: root.sessions.filter(s => /wait|input|permission|allow/i.test(String(s.status ?? ""))).length
    // the end of the current five-hour window as HH:mm, "" when no window is open
    readonly property string resetText: root.resetsAt > 0 ? Qt.formatDateTime(new Date(root.resetsAt * 1000), "HH:mm") : ""
    readonly property int percent: Math.round(root.percentage * 100)

    function refresh() {
        if (!usageProc.running) usageProc.running = true;
    }

    Timer {
        interval: Config.options.bar.aiUsage.updateInterval * 1000
        repeat: true
        running: Config.ready
        triggeredOnStart: true
        onTriggered: root.refresh()
    }

    Process {
        id: usageProc
        command: ["python3", Quickshell.shellPath("scripts/ai/claude-usage.py")]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const data = JSON.parse(text);
                    root.sessions = data.sessions;
                    root.tokens = data.tokens;
                    root.resetsAt = data.resetsAt;
                } catch (e) {}
            }
        }
    }
}
