pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.modules.common.functions

/**
 * iCloud Drive at ~/iCloud, through rclone's iclouddrive backend, and in the file manager's
 * sidebar: a GTK bookmark (GTK file choosers) and, once Dolphin has written its places file, a
 * KDE place (Dolphin and the Qt file dialogs).
 *
 * The mount runs as a transient user unit (`hecate-icloud-drive`), started at login when the
 * remote exists, so it outlives a shell restart and `systemctl --user` can see it. Files are
 * cached on disk (`--vfs-cache-mode full`), so apps can write to them like local ones;
 * rclone's remote control socket says what is still uploading.
 *
 * Signing in is rclone's own config flow, in a terminal: password and the two-factor code.
 * Apple's session runs out after a while; `signIn()` again then (rclone config reconnect).
 *
 * rclone can't reach iCloud Drive when Advanced Data Protection is on for the account: Apple
 * only hands the web API to trusted Apple devices then. Turning ADP off (iPhone: Settings →
 * your name → iCloud → Advanced Data Protection) is the only way round it.
 */
Singleton {
    id: root

    // Config.options.apple.* once Config declares it; these are the defaults.
    readonly property bool wanted: Config.options?.apple?.drive?.enable ?? false
    readonly property string remote: Config.options?.apple?.drive?.remote || "icloud"
    readonly property string mountPoint: {
        const p = Config.options?.apple?.drive?.path || "~/iCloud";
        return p.startsWith("~/") ? `${Quickshell.env("HOME")}/${p.slice(2)}` : p;
    }
    readonly property string appleId: Photos.appleId
    readonly property string unit: "hecate-icloud-drive"
    readonly property string socket: `${Quickshell.env("XDG_RUNTIME_DIR") || "/tmp"}/hecate-icloud-drive.sock`
    readonly property string adpNote: "not available with Advanced Data Protection"

    property bool installed: false
    // an iclouddrive remote called `remote` is in rclone's config
    property bool configured: false
    property bool mounted: false
    // "idle" | "mounting" | "ok" | "signIn" | "missing" | "error"
    property string status: "idle"
    property string message: ""
    // The top level of the drive: [{ name, syncing }], folders first. `syncing` counts the
    // files under it still waiting to upload.
    property var entries: []
    property int uploading: 0

    // Widgets that show the entries hold this up; the list is read only while it is above 0.
    property int consumers: 0

    function signIn() {
        const q = s => `'${String(s).replace(/'/g, "'\\''")}'`;
        const script = root.configured
            ? `rclone config reconnect ${q(root.remote + ":")}; echo; echo "done, you can close this window"; read -r _`
            : [
                `echo "iCloud Drive for ${root.appleId || "your Apple ID"}. It does not work with Advanced Data Protection."`,
                `printf "Apple ID password: "; stty -echo; read -r pw; stty echo; echo`,
                `rclone config create ${q(root.remote)} iclouddrive apple_id=${q(root.appleId)} password="$pw" --obscure`,
                `unset pw; echo; echo "done, you can close this window"; read -r _`,
            ].join("; ");
        Photos.openTerminal(script);
        root.signInPending = true;
    }
    property bool signInPending: false

    function mount() {
        if (!root.installed || !root.configured || mountProc.running) return;
        root.status = "mounting";
        mountProc.command = ["sh", "-c", root.mountScript, "drive", root.remote, root.mountPoint, root.unit, root.socket];
        mountProc.running = true;
    }

    function unmount() {
        Quickshell.execDetached(["systemctl", "--user", "stop", root.unit]);
        root.mounted = false;
        root.entries = [];
        root.status = "idle";
    }

    function open() {
        Quickshell.execDetached(["xdg-open", root.mountPoint]);
    }

    // $1 remote, $2 mount point, $3 unit, $4 rc socket.
    readonly property string mountScript: [
        'mkdir -p "$2" || { echo "error cannot create $2"; exit 0; }',
        'if ! systemctl --user is-active --quiet "$3"; then',
        '    systemctl --user reset-failed "$3" 2>/dev/null',
        '    rm -f "$4"',
        '    systemd-run --user --quiet --collect --unit="$3" -p Restart=on-failure -p RestartSec=30 \\',
        '        -p StartLimitIntervalSec=600 -p StartLimitBurst=3 \\',
        '        rclone mount "$1:" "$2" --vfs-cache-mode full --dir-cache-time 5m --poll-interval 1m \\',
        '        --rc --rc-addr "unix://$4" --rc-no-auth || { echo "error systemd-run failed"; exit 0; }',
        'fi',
        'for i in 1 2 3 4 5 6 7 8 9 10; do mountpoint -q "$2" && break; sleep 1; done',
        'if mountpoint -q "$2"; then echo ok; else echo "error $(journalctl --user -u "$3" -n 3 --no-pager -o cat 2>/dev/null | tail -n 1)"; fi',
        // The sidebar: one bookmark, added once.
        'b="${XDG_CONFIG_HOME:-$HOME/.config}/gtk-3.0/bookmarks"; mkdir -p "$(dirname "$b")"; touch "$b"',
        'grep -q "^file://$2 " "$b" 2>/dev/null || echo "file://$2 iCloud" >> "$b"',
        // Only into a places file KDE already wrote: one written here would hide its default places.
        'p="${XDG_DATA_HOME:-$HOME/.local/share}/user-places.xbel"',
        'if [ -f "$p" ] && ! grep -q "href=\\"file://$2\\"" "$p"; then',
        '    sed -i "s|</xbel>| <bookmark href=\\"file://$2\\"><title>iCloud</title><info><metadata owner=\\"http://freedesktop.org\\"><bookmark:icon name=\\"folder-cloud\\"/></metadata></info></bookmark>\\n</xbel>|" "$p"',
        'fi',
    ].join("\n")

    Process {
        id: mountProc
        stdout: StdioCollector {
            onStreamFinished: {
                const answer = text.trim().split("\n")[0] ?? "";
                if (answer === "ok") {
                    root.mounted = true;
                    root.status = "ok";
                    root.message = "";
                    root.refresh();
                } else {
                    root.mounted = false;
                    const why = answer.replace(/^error\s*/, "");
                    const auth = /auth|2fa|two-factor|401|403|token|session/i.test(why);
                    root.status = auth ? "signIn" : "error";
                    root.message = auth ? "sign in to iCloud again" : why;
                }
            }
        }
    }

    // ── state ─────────────────────────────────────────────────────────────────────

    // Which tools exist, whether the remote is configured, whether the unit is up.
    readonly property string probeScript: [
        'command -v rclone >/dev/null || { echo missing; exit 0; }',
        'rclone listremotes --long 2>/dev/null | grep -q "^$1:[[:space:]]*iclouddrive" && echo configured',
        'mountpoint -q "$2" && echo mounted',
    ].join("\n")

    function probe() {
        if (!probeProc.running) {
            probeProc.command = ["sh", "-c", root.probeScript, "drive", root.remote, root.mountPoint];
            probeProc.running = true;
        }
    }

    Process {
        id: probeProc
        stdout: StdioCollector {
            onStreamFinished: {
                const words = text.split("\n").map(l => l.trim());
                root.installed = !words.includes("missing");
                root.configured = words.includes("configured");
                root.mounted = words.includes("mounted");
                if (!root.installed) root.status = "missing";
                else if (root.mounted) root.status = "ok";
                if (root.wanted && root.configured && !root.mounted && root.status !== "signIn" && root.status !== "error")
                    root.mount();
                if (root.mounted) root.refresh();
            }
        }
    }

    // The top-level folders and files, and the upload queue from rclone's rc socket.
    readonly property string listScript: [
        'cd "$1" 2>/dev/null || exit 0',
        'for f in * .[!.]*; do [ -e "$f" ] || continue; if [ -d "$f" ]; then echo "d $f"; else echo "f $f"; fi; done',
        'echo "::queue"',
        'rclone rc --unix-socket "$2" vfs/queue 2>/dev/null',
    ].join("\n")

    function refresh() {
        if (!root.mounted || listProc.running) return;
        listProc.command = ["sh", "-c", root.listScript, "drive", root.mountPoint, root.socket];
        listProc.running = true;
    }

    Process {
        id: listProc
        stdout: StdioCollector {
            onStreamFinished: {
                const all = text.split("\n");
                const marker = all.indexOf("::queue");
                const lines = (marker >= 0 ? all.slice(0, marker) : all).filter(l => /^[df] /.test(l));
                let queue = [];
                try {
                    queue = JSON.parse(all.slice(marker + 1).join("\n") || "{}").queue ?? [];
                } catch (e) {
                    queue = [];
                }
                const counts = {};
                for (const item of queue) {
                    const top = String(item.name ?? "").split("/")[0];
                    counts[top] = (counts[top] ?? 0) + 1;
                }
                root.uploading = queue.length;
                root.entries = lines
                    .map(l => ({ name: l.slice(2), dir: l[0] === "d", syncing: counts[l.slice(2)] ?? 0 }))
                    .sort((a, b) => (a.dir === b.dir) ? a.name.localeCompare(b.name) : (a.dir ? -1 : 1));
            }
        }
    }

    Timer {
        running: root.mounted
        interval: root.consumers > 0 ? 15000 : 5 * 60 * 1000
        repeat: true
        triggeredOnStart: true
        onTriggered: root.refresh()
    }

    // After the sign-in terminal closes, rclone's config has changed; look again.
    Timer {
        running: root.signInPending
        interval: 5000
        repeat: true
        onTriggered: {
            if (configWatch.changed) {
                configWatch.changed = false;
                root.signInPending = false;
                root.status = "idle";
                root.probe();
            }
        }
    }

    FileView {
        id: configWatch
        property bool changed: false
        path: `${FileUtils.trimFileProtocol(Directories.config)}/rclone/rclone.conf`
        watchChanges: true
        onFileChanged: {
            configWatch.changed = true;
            reload();
        }
    }

    onWantedChanged: root.wanted ? root.probe() : root.unmount()
    Component.onCompleted: if (root.wanted) root.probe()

    function load() {} // for forcing initialisation

    IpcHandler {
        target: "icloudDrive"

        function mount(): void {
            root.mount();
        }
        function unmount(): void {
            root.unmount();
        }
        function status(): string {
            return JSON.stringify({ status: root.status, message: root.message, mounted: root.mounted,
                configured: root.configured, uploading: root.uploading });
        }
    }
}
