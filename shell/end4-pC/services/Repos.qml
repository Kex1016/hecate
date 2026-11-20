pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common

/**
 * Git status of the checkouts under ~/src, for the repos widget.
 *
 * All git work runs in a child shell (one `git status` per checkout, and now and then a
 * `git fetch` so "behind" means something); the shell only parses its output, line by line,
 * so a slow disk or a hung remote never blocks the UI thread. Nothing runs while
 * `consumers` is 0.
 */
Singleton {
    id: root

    // Checkouts are the directories under `path` that hold a .git, and one level further down
    // for directories that are not checkouts themselves (~/src/github/foo).
    readonly property string path: {
        const p = Config.options?.repos?.path || "~/src";
        return p.startsWith("~/") ? `${Quickshell.env("HOME")}/${p.slice(2)}` : p;
    }
    readonly property int interval: 60 * 1000
    // Fetch remotes this often so "behind" is current. Fetches never prompt: no terminal
    // prompt, ssh in batch mode, 30 s per checkout at most.
    readonly property bool fetchRemotes: Config.options?.repos?.fetch ?? true
    readonly property int fetchInterval: 30 * 60 * 1000

    property int consumers: 0
    readonly property bool active: consumers > 0
    readonly property bool loading: statusProc.running
    readonly property bool fetching: fetchProc.running
    // false until the first scan, or when `path` does not exist
    property bool available: false

    // [{ name, path, branch, changed, ahead, behind, upstream, state }], the ones that need you
    // first. state is "changed" (uncommitted work), "ahead" (commits not pushed), "behind"
    // (upstream has commits you lack) or "clean".
    property var repos: []
    readonly property int needingYou: repos.filter(r => r.state !== "clean").length

    property var pending: []
    property double lastFetch: 0

    function refresh() {
        if (!statusProc.running) {
            root.pending = [];
            statusProc.command = ["sh", "-c", root.script, "repos", root.path, "status"];
            statusProc.running = true;
        }
        if (root.fetchRemotes && !fetchProc.running && Date.now() - root.lastFetch >= root.fetchInterval) {
            root.lastFetch = Date.now();
            fetchProc.command = ["sh", "-c", root.script, "repos", root.path, "fetch"];
            fetchProc.running = true;
        }
    }

    function openTerminal(repoPath) {
        Quickshell.execDetached({
            command: ["sh", "-c", Config.options.apps.terminal],
            workingDirectory: repoPath,
        });
    }

    function stateOf(r) {
        if (r.changed > 0) return "changed";
        if (r.ahead > 0) return "ahead";
        if (r.behind > 0) return "behind";
        return "clean";
    }

    function parseLine(line) {
        if (line === "nodir") {
            root.available = false;
            return;
        }
        const f = line.split("\t");
        if (f.length < 6) return;
        const r = {
            path: f[0],
            name: f[0].startsWith(root.path + "/") ? f[0].slice(root.path.length + 1) : f[0],
            branch: f[1] === "(detached)" ? "detached" : f[1],
            changed: parseInt(f[2]) || 0,
            ahead: parseInt(f[3]) || 0,
            behind: parseInt(f[4]) || 0,
            upstream: f[5],
        };
        r.state = root.stateOf(r);
        root.pending.push(r);
    }

    function finish() {
        const order = { changed: 0, ahead: 1, behind: 2, clean: 3 };
        root.repos = root.pending.slice().sort((a, b) =>
            order[a.state] - order[b.state] || a.name.localeCompare(b.name));
        root.available = root.available || root.repos.length > 0;
        root.pending = [];
    }

    // $1: the directory; $2: "status" prints one tab-separated line per checkout (path,
    // branch, changed entries with untracked ones, ahead, behind, upstream), "fetch" only
    // fetches every checkout's remotes and prints nothing.
    readonly property string script: `
        dir=$1; mode=$2
        [ -d "$dir" ] || { echo nodir; exit 0; }
        list() {
            for d in "$dir"/*/; do
                d=\${d%/}
                if [ -e "$d/.git" ]; then echo "$d"
                else for e in "$d"/*/; do e=\${e%/}; [ -e "$e/.git" ] && echo "$e"; done
                fi
            done
        }
        list | while IFS= read -r d; do
            if [ "$mode" = fetch ]; then
                GIT_TERMINAL_PROMPT=0 GIT_SSH_COMMAND="ssh -o BatchMode=yes -o ConnectTimeout=10" \\
                    timeout 30 git -C "$d" fetch --quiet --no-tags --no-recurse-submodules </dev/null >/dev/null 2>&1
                continue
            fi
            git -C "$d" --no-optional-locks status --porcelain=v2 --branch 2>/dev/null | awk -v d="$d" '
                /^# branch.head / { head = $3 }
                /^# branch.upstream / { up = $3 }
                /^# branch.ab / { a = substr($3, 2); b = substr($4, 2) }
                /^[^#]/ { n++ }
                END { if (head != "") printf "%s\\t%s\\t%d\\t%d\\t%d\\t%s\\n", d, head, n, a, b, up }'
        done
    `

    Process {
        id: statusProc
        environment: ({ LANG: "C", LC_ALL: "C", GIT_OPTIONAL_LOCKS: "0" })
        stdout: SplitParser {
            onRead: line => root.parseLine(line)
        }
        onExited: (exitCode, exitStatus) => root.finish()
    }

    Process {
        id: fetchProc
        environment: ({ LANG: "C", LC_ALL: "C" })
        // new remote refs change "behind": read the status again
        onExited: (exitCode, exitStatus) => root.refresh()
    }

    Timer {
        running: root.active
        repeat: true
        triggeredOnStart: true
        interval: root.interval
        onTriggered: root.refresh()
    }
}
