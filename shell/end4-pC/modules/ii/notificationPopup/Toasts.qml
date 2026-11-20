pragma Singleton
import QtQuick
import Quickshell

// Small helpers the toast stack shares: relative ages on one minute-ish
// clock, markup stripping, and the permission-request test.
Singleton {
    id: root

    property double now: Date.now()

    Timer {
        interval: 15000
        running: true
        repeat: true
        onTriggered: root.now = Date.now()
    }

    // the service keeps the enum as its string form: "2" for critical
    function isCritical(urgency) {
        return String(urgency) === "2" || /critical/i.test(String(urgency));
    }

    function age(time, now) {
        const s = Math.max(0, Math.floor((now - time) / 1000));
        if (s < 60) return "now";
        if (s < 3600) return Math.floor(s / 60) + "m";
        if (s < 86400) return Math.floor(s / 3600) + "h";
        return Math.floor(s / 86400) + "d";
    }

    // body text arrives with markup; the haiiro face draws plain text
    function plain(s) {
        return (s ?? "").replace(/<br\s*\/?>/gi, "\n").replace(/<[^>]+>/g, "")
            .replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, "\"")
            .replace(/&#39;|&apos;/g, "'").replace(/&amp;/g, "&").trim();
    }

    // a request that needs an answer: the `permission` category (Claude Code
    // hooks send it), or Claude asking whether it may do something
    function isPermission(appName, summary, category) {
        if (category === "permission" || category === "permission.request") return true;
        return /claude/i.test(appName) && /permission|allow|approve/i.test(summary);
    }
}
