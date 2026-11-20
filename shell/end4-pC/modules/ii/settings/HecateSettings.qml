pragma Singleton
pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common

// The compositor settings the hecate pages change. Only what you change is
// kept: the edits live in ~/.config/hecate/shell.json and are written out as
// ~/.config/hecate/shell.toml, which the Home Manager module includes after
// its own settings, so they win over the Nix config.
//
// Frames, window motion and blur are haiiro's and come from the Home Manager
// module. The shell never writes `border.*`, `animations.*` or `blur.*`: an
// old shell.toml carrying them would override the haiiro defaults, so any it
// finds are dropped and the file is rewritten without them.
Singleton {
    id: root

    readonly property string dir: Directories.hecateConfig
    readonly property list<string> forbidden: ["border.", "animations.", "blur."]

    // what `hecate msg -j config` reports, and the paths edited here ("general.gaps" -> 8)
    property var effective: ({})
    property var edits: ({})
    property bool loaded: false
    // `hecate msg -j outputs`, sorted by name
    property var outputs: []

    function allowed(path) {
        return !root.forbidden.some(p => path.startsWith(p));
    }

    function value(path, fallback) {
        if (root.edits[path] !== undefined) return root.edits[path];
        const v = root.lookup(root.effective, path);
        return v === undefined || v === null ? fallback : v;
    }

    // Paths are dotted; a part may hold a colon ("input.type:pointer.accel_profile").
    function lookup(obj, path) {
        return path.split(".").reduce((o, k) => o?.[k], obj);
    }

    function set(path, v) {
        if (!root.loaded || !root.allowed(path)) return;
        if (JSON.stringify(root.value(path, undefined)) === JSON.stringify(v)) return;
        const e = Object.assign({}, root.edits);
        e[path] = v;
        root.edits = e;
        root.save();
    }

    // Drop an edit, so the Nix config (or hecate's default) applies again.
    function unset(path) {
        if (!root.loaded || root.edits[path] === undefined) return;
        const e = Object.assign({}, root.edits);
        delete e[path];
        root.edits = e;
        root.save();
    }

    function reset() {
        resetProc.running = true;
    }

    function tomlKey(k) {
        return /^[A-Za-z0-9_-]+$/.test(k) ? k : JSON.stringify(k);
    }

    function toToml(edits) {
        const sections = {};
        for (const [path, v] of Object.entries(edits)) {
            const parts = path.split(".").map(root.tomlKey);
            const key = parts.pop();
            const section = parts.join(".");
            (sections[section] = sections[section] ?? []).push(`${key} = ${JSON.stringify(v)}`);
        }
        let out = "# Written by the shell's hecate settings pages. Edit there, or delete this file.\n";
        for (const [section, lines] of Object.entries(sections))
            out += (section ? `\n[${section}]\n` : "\n") + lines.join("\n") + "\n";
        return out;
    }

    function save() {
        mkdirProc.running = true;
        jsonFile.setText(JSON.stringify(root.edits, null, 2));
        tomlFile.setText(root.toToml(root.edits));
        reloadTimer.restart();
    }

    function refresh() {
        configProc.running = true;
        outputsProc.running = true;
    }

    // The exact mode string hecate writes for a mode, so it picks that mode again.
    function modeValue(m) {
        return `${m.width}x${m.height}@${(m.refresh_rate / 1000).toFixed(3)}`;
    }
    function modeLabel(m) {
        return `${m.width}×${m.height} ${Math.round(m.refresh_rate / 1000)}Hz`;
    }
    // Every mode of an output, largest and fastest first.
    function modes(o) {
        const seen = {};
        return (o?.modes ?? []).filter(m => {
            const v = root.modeValue(m);
            if (seen[v]) return false;
            seen[v] = true;
            return true;
        }).sort((a, b) => (b.width * b.height - a.width * a.height) || (b.refresh_rate - a.refresh_rate));
    }
    function currentMode(o) {
        const edited = root.edits[`outputs.${o.name}.mode`];
        if (edited !== undefined) return edited;
        const m = o.current_mode !== null && o.current_mode !== undefined ? o.modes[o.current_mode] : null;
        return m ? root.modeValue(m) : "";
    }

    Component.onCompleted: root.refresh()

    Process {
        id: mkdirProc
        command: ["mkdir", "-p", root.dir]
    }
    Timer {
        id: reloadTimer
        interval: 300
        onTriggered: reloadProc.running = true
    }
    Process {
        id: reloadProc
        command: ["hecate", "msg", "reload"]
        // A new mode changes what the outputs report.
        onExited: outputsTimer.restart()
    }
    Timer {
        id: outputsTimer
        interval: 500
        onTriggered: outputsProc.running = true
    }
    Process {
        id: outputsProc
        command: ["hecate", "msg", "-j", "outputs"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const map = JSON.parse(text);
                    root.outputs = Object.keys(map).sort().map(k => Object.assign({ name: k }, map[k]));
                } catch (e) {
                    root.outputs = [];
                }
            }
        }
    }
    Process {
        id: configProc
        command: ["hecate", "msg", "-j", "config"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.effective = JSON.parse(text);
                } catch (e) {
                    root.effective = {};
                }
            }
        }
    }

    FileView {
        id: jsonFile
        path: `${root.dir}/shell.json`
        printErrors: false
        onLoaded: {
            let e = {};
            try {
                e = JSON.parse(jsonFile.text()) ?? {};
            } catch (err) {
                e = {};
            }
            const kept = {};
            for (const [k, v] of Object.entries(e))
                if (root.allowed(k)) kept[k] = v;
            const dropped = Object.keys(e).length !== Object.keys(kept).length;
            root.edits = kept;
            root.loaded = true;
            // Stale frame and motion keys from the old settings page: rewrite without them.
            if (dropped) root.save();
        }
        onLoadFailed: {
            root.loaded = true;
            // No edits of ours, but an old shell.toml may still carry frame or motion keys.
            staleToml.path = `${root.dir}/shell.toml`;
        }
    }
    FileView {
        id: tomlFile
        path: `${root.dir}/shell.toml`
        printErrors: false
    }
    // Only read once shell.json turned out missing: read at startup it would
    // race the edits and save over them.
    FileView {
        id: staleToml
        printErrors: false
        // Drops the frame, motion and blur tables and keeps the rest as it was.
        onLoaded: {
            const text = staleToml.text();
            if (!/^\s*\[(border|animations|blur)\b/m.test(text)) return;
            let skip = false;
            const kept = text.split("\n").filter(line => {
                const table = line.match(/^\s*\[\[?\s*"?([A-Za-z0-9_-]+)/);
                if (table) skip = ["border", "animations", "blur"].includes(table[1]);
                return !skip;
            });
            tomlFile.setText(kept.join("\n"));
            reloadTimer.restart();
        }
    }

    Process {
        id: resetProc
        command: ["sh", "-c", `rm -f "${root.dir}/shell.toml" "${root.dir}/shell.json" && hecate msg reload`]
        onExited: {
            root.edits = {};
            root.refresh();
        }
    }
}
