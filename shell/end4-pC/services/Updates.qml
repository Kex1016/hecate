pragma Singleton
import qs
import qs.modules.common
import qs.modules.common.functions
import QtQuick
import Quickshell
import Quickshell.Io

/*
 * System updates service. Arch (pacman + AUR), or CakeOS (hecate): when /etc/cakeos holds the
 * flake and `update-cakeos` exists, updates are upstream CakeOS commits not yet installed, and
 * the updater is the shell's own dialog (modules/ii/cakeosUpdater) running
 * `update-cakeos apply` through sudo.
 */
Singleton {
    id: root

    property bool available: false
    property alias checking: checkUpdatesProc.running
    property int count: 0

    readonly property bool updateAdvised: available && count > Config.options.updates.adviseUpdateThreshold
    readonly property bool updateStronglyAdvised: available && count > Config.options.updates.stronglyAdviseUpdateThreshold

    // --- CakeOS -----------------------------------------------------------------------------
    readonly property bool cakeos: root.cakeosUpdater !== ""
    // Path of `update-cakeos`, "" when this isn't CakeOS.
    property string cakeosUpdater: ""
    // Last `update-cakeos check --json`: installed, branch, latest, behind (null: unknown
    // version), commits [{ sha, subject, author, date }]; or { error }.
    property var cakeosStatus: null
    property bool updaterOpen: false
    // The running (or last) `update-cakeos apply`.
    readonly property alias applying: applyProc.running
    property var applyLog: []
    property string applyStatus: ""
    // "", "ok", "failed" or "wrongPassword".
    property string applyResult: ""
    property string applyMode: "boot"
    // nu (the script's interpreter) and efibootmgr-style user tools may only be in a profile.
    readonly property string toolPath: `${Quickshell.env("PATH")}:/run/current-system/sw/bin:/etc/profiles/per-user/${Quickshell.env("USER")}/bin:${Quickshell.env("HOME")}/.nix-profile/bin`

    function load() {}
    function refresh() {
        if (!available) return;
        print("[Updates] Checking for system updates")
        checkUpdatesProc.running = true;
    }

    function openUpdater() {
        root.updaterOpen = true;
        if (!root.applying && root.applyResult !== "") root.applyResult = "";
        if (!root.checking && !root.applying) root.refresh();
    }

    function apply(password, mode) {
        if (!root.cakeos || root.applying) return;
        root.applyLog = [];
        root.applyStatus = Translation.tr("Starting…");
        root.applyResult = "";
        root.applyMode = mode;
        applyProc.password = password;
        applyProc.command = ["sudo", "-S", "-k", "-p", "", "env", `PATH=${root.toolPath}`,
            root.cakeosUpdater, "apply", "--mode", mode];
        applyProc.running = true;
    }

    function appendLog(line) {
        const log = root.applyLog.concat([line]);
        root.applyLog = log.length > 400 ? log.slice(log.length - 400) : log;
        if (line.startsWith("==> ")) root.applyStatus = line.slice(4);
    }

    Timer {
        interval: Config.options.updates.checkInterval * 60 * 1000
        repeat: true
        running: Config.ready && Config.options.updates.enableCheck
        onTriggered: {
            print("[Updates] Periodic update check due")
            root.refresh();
        }
    }

    Process {
        id: checkAvailabilityProc
        running: Config.ready && Config.options.updates.enableCheck
        command: ["sh", "-c", `PATH="${root.toolPath}"
            u=$(command -v update-cakeos || echo "$HOME/.local/bin/update-cakeos")
            if [ -f /etc/cakeos/flake.nix ] && [ -x "$u" ]; then echo "cakeos $u"; exit 0; fi
            command -v checkupdates >/dev/null && echo arch`]
        stdout: StdioCollector {
            onStreamFinished: {
                const out = text.trim();
                root.cakeosUpdater = out.startsWith("cakeos ") ? out.slice(7) : "";
                root.available = out !== "";
                // CakeOS checks talk to GitHub; give the network a moment after login.
                if (root.available) firstCheckTimer.start();
            }
        }
    }

    Timer {
        id: firstCheckTimer
        interval: root.cakeos ? 15 * 1000 : 60 * 1000
        onTriggered: root.refresh()
    }

    Process {
        id: checkUpdatesProc
        command: root.cakeos
            ? ["env", `PATH=${root.toolPath}`, root.cakeosUpdater, "check", "--json"]
            : ["bash", "-c", "pacman=$(checkupdates 2>/dev/null | wc -l); aur=$(yay -Qua 2>/dev/null | wc -l || paru -Qua 2>/dev/null | wc -l || echo 0); echo $((pacman + aur))"]
        stdout: StdioCollector {
            onStreamFinished: {
                if (!root.cakeos) {
                    root.count = parseInt(text.trim());
                    return;
                }
                try {
                    const status = JSON.parse(text.trim());
                    root.cakeosStatus = status;
                    // An unknown installed version counts as one update: updating records it.
                    if (!status.error) root.count = status.behind ?? 1;
                } catch (e) {
                    root.cakeosStatus = { error: Translation.tr("Couldn't read the update check") };
                }
            }
        }
    }

    Process {
        id: applyProc
        property string password: ""
        stdinEnabled: true
        onStarted: {
            applyProc.write(applyProc.password + "\n");
            applyProc.password = "";
            applyProc.stdinEnabled = false;
        }
        stdout: SplitParser {
            onRead: line => root.appendLog(line)
        }
        stderr: SplitParser {
            onRead: line => root.appendLog(line)
        }
        onExited: (exitCode, exitStatus) => {
            applyProc.stdinEnabled = true;
            if (exitCode === 0) {
                root.applyResult = "ok";
                root.applyStatus = root.applyMode === "boot"
                    ? Translation.tr("Restart to finish the update")
                    : Translation.tr("CakeOS is up to date");
            } else if (root.applyLog.some(l => /incorrect password|try again|no password was provided/i.test(l))) {
                root.applyResult = "wrongPassword";
            } else {
                root.applyResult = "failed";
                const error = root.applyLog.slice().reverse().find(l => l.startsWith("!!! ") || /^error:/i.test(l));
                root.applyStatus = error ? error.replace(/^!!! /, "") : Translation.tr("The update failed (exit code %1)").arg(exitCode);
            }
            root.refresh();
        }
    }
}
