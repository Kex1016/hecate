import QtQuick
import Quickshell

// Tints the border of every window whose Claude session waits for an answer
// gold, and clears it when the answer is in:
//
//   hecate msg tint gold --ms 20000 --id <window>      (or --pid <terminal> when the window is not known)
//   hecate msg tint none --id <window>
//
// The tint is a lease: it runs out after `leaseMs` unless it is renewed, which
// happens every `renewMs` while the session still waits, so a shell that dies
// or reloads cannot leave a window gold for good.
Scope {
    id: root

    // what to tint, as "id:<window id>" or "pid:<process id>" (the compositor finds the
    // window of a pid)
    property var targets: []
    property bool enabled: true

    readonly property int leaseMs: 20000
    readonly property int renewMs: 8000

    property var tinted: ({})

    function msg(args) {
        Quickshell.execDetached(["hecate", "msg"].concat(args));
    }

    function flags(target) {
        const k = String(target).split(":");
        return ["--" + k[0], k.slice(1).join(":")];
    }

    function apply() {
        const want = {};
        if (root.enabled)
            for (const t of root.targets)
                if (/^(id|pid):[0-9]+$/.test(String(t)) && Number(String(t).split(":")[1]) > 0) want[String(t)] = true;
        for (const t of Object.keys(want))
            if (!root.tinted[t]) root.msg(["tint", "gold", "--ms", String(root.leaseMs)].concat(root.flags(t)));
        for (const t of Object.keys(root.tinted))
            if (!want[t]) root.msg(["tint", "none"].concat(root.flags(t)));
        root.tinted = want;
    }

    onTargetsChanged: apply()
    onEnabledChanged: apply()

    Timer {
        interval: root.renewMs
        repeat: true
        running: Object.keys(root.tinted).length > 0
        onTriggered: {
            for (const t of Object.keys(root.tinted))
                root.msg(["tint", "gold", "--ms", String(root.leaseMs)].concat(root.flags(t)));
        }
    }

    Component.onDestruction: {
        for (const t of Object.keys(root.tinted))
            root.msg(["tint", "none"].concat(root.flags(t)));
    }
}
