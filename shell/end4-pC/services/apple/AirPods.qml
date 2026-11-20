pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.services

/**
 * AirPods, through librepods (CakeOS: `cakeos.hardware = [ "airpods" ]`).
 *
 * librepods talks to the AirPods over Apple's own L2CAP protocol; nothing else on Linux reads
 * each bud's and the case's battery or sets the noise mode. It has no API for any of that, so
 * the shell runs it (`librepods --hide --debug`, in the tray as usual) and reads what it logs:
 * battery, which bud is where, the noise mode, the name. Setting the noise mode goes back
 * through `librepods-ctl`, its own socket client.
 *
 * Only one librepods can run. When one was already started some other way, `external` is
 * true: the noise chips still work, the battery stays unknown until `takeOver()` restarts it
 * here.
 *
 * Pausing when a bud comes out: librepods does it itself, but only when the AirPods are the
 * PulseAudio default sink and its ear-detection setting is on. The shell pauses the player it
 * shows as well (`pauseOnRemove`), the moment either bud leaves an ear while both were in.
 * Putting it back in doesn't resume anything.
 */
Singleton {
    id: root

    // Config.options.apple.airpods.* once Config declares it; these are the defaults.
    readonly property bool wanted: Config.options?.apple?.airpods?.enable ?? true
    readonly property bool pauseOnRemove: Config.options?.apple?.airpods?.pauseOnRemove ?? true

    // librepods and librepods-ctl are on PATH
    property bool installed: false
    // another librepods (autostart, a launcher) holds the AirPods' socket
    property bool external: false
    readonly property bool running: daemon.running
    property bool connected: false

    property string name: ""
    property string modelNumber: ""
    // 0-100, -1 while unknown. AirPods Max report one level, `headset`.
    property int left: -1
    property int right: -1
    property int caseLevel: -1
    property int headset: -1
    readonly property bool isHeadset: headset >= 0 && left < 0 && right < 0
    // "inEar" | "out" | "inCase" | "unknown"
    property string leftState: "unknown"
    property string rightState: "unknown"
    readonly property bool leftInEar: leftState === "inEar"
    readonly property bool rightInEar: rightState === "inEar"
    readonly property bool anyInEar: leftInEar || rightInEar
    // "off" | "anc" | "transparency" | "adaptive" | "" while unknown
    property string noiseMode: ""

    // What the bar shows: the lower of the two buds (or the headset).
    readonly property int lowerBud: {
        if (root.isHeadset) return root.headset;
        const known = [root.left, root.right].filter(v => v >= 0);
        return known.length > 0 ? Math.min(...known) : -1;
    }
    readonly property string lowerSide: root.isHeadset ? "headset"
        : (root.left >= 0 && (root.right < 0 || root.left <= root.right)) ? "left" : (root.right >= 0 ? "right" : "")

    // The chips: id, label (the board's words), librepods-ctl argument.
    readonly property var modes: [
        { id: "off", label: "off", ctl: "noise:off" },
        { id: "anc", label: "noise cancel", ctl: "noise:anc" },
        { id: "transparency", label: "transparency", ctl: "noise:transparency" },
        { id: "adaptive", label: "adaptive", ctl: "noise:adaptive" },
    ]

    function setNoiseMode(id) {
        const mode = root.modes.find(m => m.id === id);
        if (!mode || !(root.running || root.external)) return;
        // Shown at once; librepods logs the AirPods' answer and that confirms or corrects it.
        root.noiseMode = id;
        Quickshell.execDetached(["librepods-ctl", mode.ctl]);
    }

    // Open librepods' own window (hearing aid, conversational awareness, renaming).
    function openApp() {
        Quickshell.execDetached(["librepods-ctl", "reopen"]);
    }

    // Stop the librepods started elsewhere and run it here, so the battery shows.
    function takeOver() {
        if (!root.external) return;
        takeOverProc.running = true;
    }

    function load() {} // for forcing initialisation

    function reset() {
        root.connected = false;
        root.left = -1;
        root.right = -1;
        root.caseLevel = -1;
        root.headset = -1;
        root.leftState = "unknown";
        root.rightState = "unknown";
        root.noiseMode = "";
    }

    // ── reading librepods' log ─────────────────────────────────────────────────────
    // qDebug formatting: QStrings come quoted, enums with their scope, colour codes around
    // every line. Each pattern only looks for the words that matter.

    property string primarySide: "left"
    readonly property var stateNames: ({ InEar: "inEar", NotInEar: "out", InCase: "inCase", Disconnected: "unknown" })
    readonly property var noiseNames: ({ Off: "off", NoiseCancellation: "anc", Transparency: "transparency", Adaptive: "adaptive",
        "0": "off", "1": "anc", "2": "transparency", "3": "adaptive" })

    function parseLine(raw) {
        const line = raw.replace(/\x1b\[[0-9;]*m/g, "").replace(/"/g, "").trim();
        let m;
        if ((m = line.match(/Battery status:\s*Left:\s*(-?\d+)%,\s*Right:\s*(-?\d+)%,\s*Case:\s*(-?\d+)%/))) {
            root.connected = true;
            root.left = root.level(m[1]);
            root.right = root.level(m[2]);
            root.caseLevel = root.level(m[3]);
            root.headset = -1;
        } else if ((m = line.match(/Battery status:\s*Headset:\s*(-?\d+)%/))) {
            root.connected = true;
            root.headset = root.level(m[1]);
            root.left = -1;
            root.right = -1;
            root.caseLevel = -1;
        } else if ((m = line.match(/Primary Pod:\s*\S*?(Left|Right|Headset)\b/))) {
            root.primarySide = m[1] === "Right" ? "right" : "left";
        } else if ((m = line.match(/Primary -\s*\S*?(NotInEar|InEar|InCase|Disconnected)\s*,\s*Secondary -\s*\S*?(NotInEar|InEar|InCase|Disconnected)/))) {
            const primary = root.stateNames[m[1]];
            const secondary = root.stateNames[m[2]];
            root.setEars(root.primarySide === "left" ? primary : secondary, root.primarySide === "left" ? secondary : primary);
        } else if ((m = line.match(/Noise control mode (?:received|is already set to):\s*\S*?(NoiseCancellation|Transparency|Adaptive|Off|\d)\b/))) {
            root.noiseMode = root.noiseNames[m[1]] ?? root.noiseMode;
        } else if ((m = line.match(/Device Name:\s*(.+)$/))) {
            root.name = m[1].trim();
        } else if ((m = line.match(/Model Number:\s*(.+)$/))) {
            root.modelNumber = m[1].trim();
        } else if (/Connected to device, sending initial packets/.test(line)) {
            root.connected = true;
        } else if (/Device disconnected:/.test(line)) {
            root.reset();
        }
    }

    function level(text) {
        const v = parseInt(text);
        return isNaN(v) || v < 0 || v > 100 ? -1 : v;
    }

    function setEars(leftState, rightState) {
        const bothWereIn = root.leftInEar && root.rightInEar;
        root.leftState = leftState;
        root.rightState = rightState;
        root.connected = true;
        if (bothWereIn && !(root.leftInEar && root.rightInEar))
            root.budRemoved();
    }

    signal budRemoved()
    onBudRemoved: {
        if (!root.pauseOnRemove) return;
        const player = MprisController.activePlayer;
        if (player && player.isPlaying && player.canPause)
            player.pause();
    }

    // ── running librepods ──────────────────────────────────────────────────────────

    readonly property bool shouldRun: root.wanted && root.installed && !root.external

    // librepods' own process name is `.librepods-wrapped` on NixOS (wrapQtAppsHook), plain
    // `librepods` elsewhere; librepods-ctl must not count.
    readonly property string probeScript: 'command -v librepods >/dev/null && command -v librepods-ctl >/dev/null || { echo missing; exit 0; }\n'
        + 'if pgrep -u "$(id -u)" -f "(^|/)\\.?librepods(-wrapped)?( |$)" >/dev/null; then echo external; else echo free; fi'

    Process {
        id: probe
        command: ["sh", "-c", root.probeScript]
        stdout: StdioCollector {
            onStreamFinished: {
                // Our own librepods would look like someone else's.
                if (daemon.running) return;
                const answer = text.trim();
                root.installed = answer !== "missing";
                root.external = answer === "external";
                if (root.shouldRun)
                    daemon.running = true;
            }
        }
    }

    Process {
        id: daemon
        // stderr into stdout; Qt must log to it rather than the journal, and debug lines are
        // where the ear detection is.
        command: ["sh", "-c", "exec librepods --hide --debug 2>&1"]
        environment: ({
            QT_FORCE_STDERR_LOGGING: "1",
            QT_LOGGING_RULES: "librepods.*=true",
        })
        stdout: SplitParser {
            onRead: data => root.parseLine(data)
        }
        onRunningChanged: {
            if (running) {
                restartTimer.attempts = 0;
                return;
            }
            root.reset();
            if (root.shouldRun) restartTimer.restart();
        }
    }

    // librepods exits when its Bluetooth adapter goes away; try again, slower each time.
    Timer {
        id: restartTimer
        property int attempts: 0
        interval: Math.min(5 * 60 * 1000, 5000 * Math.pow(2, attempts))
        onTriggered: {
            attempts += 1;
            probe.running = true;
        }
    }

    Process {
        id: takeOverProc
        command: ["sh", "-c", 'pkill -u "$(id -u)" -f "(^|/)\\.?librepods(-wrapped)?( |$)"; sleep 1']
        onExited: {
            root.external = false;
            probe.running = true;
        }
    }

    onWantedChanged: {
        if (root.wanted) probe.running = true;
        else daemon.running = false;
    }
    Component.onCompleted: if (root.wanted) probe.running = true

    IpcHandler {
        target: "airpods"

        function status(): string {
            return JSON.stringify({ connected: root.connected, left: root.left, right: root.right, case: root.caseLevel,
                headset: root.headset, leftState: root.leftState, rightState: root.rightState, noise: root.noiseMode });
        }
        function noise(mode: string): void {
            root.setNoiseMode(mode);
        }
    }
}
