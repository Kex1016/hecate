pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.modules.common.functions

/**
 * Hands your look to hecate-greetd, the login screen. It can't read your home directory, so
 * `hecate-greetd sync` copies what it needs (shell config, generated colours, wallpapers,
 * avatar, monitor layout, keyboard and mouse settings) to /var/lib/hecate-greetd/users/$USER.
 * Runs at startup and shortly after any of those change. Settings → hecate-greetd.
 */
Singleton {
    id: root

    // `hecate-greetd` is on PATH (the NixOS module installs it).
    property bool available: false
    property bool syncing: false
    // The last sync's report (`hecate-greetd sync --json`), or its error.
    property var status: ({})
    property string error: ""

    property bool _pending: false

    function load() {
        checkProc.running = true;
    }

    // Sync soon; changes come in bursts (a wallpaper change rewrites colours, config, ...).
    function request() {
        if (root.available)
            debounce.restart();
    }

    function syncNow() {
        debounce.stop();
        if (!root.available) return;
        if (syncProc.running) {
            root._pending = true;
            return;
        }
        root.syncing = true;
        syncProc.running = true;
    }

    // Run the login screen against your current look, on this session, until Escape.
    function preview() {
        if (root.available)
            Quickshell.execDetached(["hecate-greetd", "preview"]);
    }

    Timer {
        id: debounce
        interval: 2000
        onTriggered: root.syncNow()
    }

    Process {
        id: checkProc
        command: ["sh", "-c", "command -v hecate-greetd"]
        onExited: exitCode => {
            root.available = exitCode === 0;
            root.request();
        }
    }

    Process {
        id: syncProc
        command: ["hecate-greetd", "sync", "--json"]
        stdout: StdioCollector {
            id: syncOut
        }
        stderr: StdioCollector {
            id: syncErr
        }
        onExited: exitCode => {
            root.syncing = false;
            if (exitCode === 0) {
                try {
                    root.status = JSON.parse(syncOut.text);
                } catch (e) {
                    root.status = {};
                }
                root.error = "";
            } else {
                root.error = syncErr.text.trim() || `hecate-greetd sync exited with ${exitCode}`;
            }
            if (root._pending) {
                root._pending = false;
                root.syncNow();
            }
        }
    }

    // What the login screen copies; a change to any of them means a new sync.
    component Watched: FileView {
        watchChanges: true
        printErrors: false
        onFileChanged: root.request()
    }
    Watched { path: Directories.shellConfigPath }
    Watched { path: Directories.generatedMaterialThemePath }
    Watched { path: Directories.generatedLockMaterialThemePath }
    Watched { path: `${Quickshell.env("HOME")}/.face` }
    Watched { path: FileUtils.trimFileProtocol(`${Directories.config}/hecate/shell.toml`) }
    Watched { path: FileUtils.trimFileProtocol(`${Directories.config}/hecate/config.toml`) }
}
