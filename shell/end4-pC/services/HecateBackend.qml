pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io

/**
 * Compositor backend for hecate. hecate's IPC is shaped like niri's, so this mirrors
 * NiriBackend.qml, talking to `hecate msg`.
 *
 * Differences from niri: workspaces in hecate's global scope have no output (they show on
 * every monitor), and outputs report `is_focused` directly.
 */
Scope {
    id: root
    property var windowList: []
    property var workspaces: []
    property var workspaceById: ({})
    property var activeWorkspace: null
    property var monitors: []
    property var focusedMonitor: null

    function run(args) {
        Quickshell.execDetached(["hecate", "msg"].concat(args));
    }

    function switchWorkspaceRelative(direction) {
        run(["workspace", direction === "next" ? "next" : "prev"]);
    }

    function normalizeWindow(w) {
        return {
            id: String(w.id),
            address: String(w.id),
            title: w.title ?? "",
            appId: w.app_id ?? "",
            class: w.app_id ?? "",
            workspaceId: w.workspace_id,
            focused: w.is_focused ?? false,
            fullscreen: w.is_fullscreen ?? false,
            output: w.output ?? "",
            width: w.layout?.window_size?.[0] ?? 0,
            height: w.layout?.window_size?.[1] ?? 0
        };
    }

    function focusWindow(id) { run(["focus", "--id", String(id)]); }
    function closeWindow(id) { run(["close", "--id", String(id)]); }

    // Workspace indices are per output in hecate's per_output scope: focus the output first.
    function workspaceTarget(id) {
        const ws = root.workspaceById[id];
        return ws ? { idx: String(ws.idx), output: ws.output } : { idx: String(id), output: null };
    }
    function switchWorkspace(id) {
        const t = workspaceTarget(id);
        if (t.output)
            Quickshell.execDetached(["sh", "-c", `hecate msg action focus-output '${t.output}' && hecate msg workspace ${t.idx}`]);
        else
            run(["workspace", t.idx]);
    }
    function moveWindowToWorkspace(id, wsId) {
        run(["move-to-workspace", "--id", String(id), workspaceTarget(wsId).idx]);
    }

    function onMonitor(ws, monitorName) {
        // Global scope: workspaces belong to every monitor.
        return ws.output === null || ws.output === undefined || ws.output === monitorName;
    }

    function monitorFor(screen) {
        if (!screen) return null;
        return root.monitors.find(m => m.name === screen.name) ?? null;
    }

    function activeWorkspaceForMonitor(monitorName) {
        if (!monitorName) return null;
        return root.workspaces.find(ws => onMonitor(ws, monitorName) && ws.is_active) ?? null;
    }

    function biggestWindowForWorkspace(wsId) {
        const wins = root.windowList.filter(w => w.workspaceId === wsId);
        if (wins.length === 0) return null;
        return wins.reduce((a, b) => (a.width * a.height >= b.width * b.height ? a : b));
    }

    function fullscreenOnMonitor(monitorName) {
        const ws = activeWorkspaceForMonitor(monitorName);
        if (!ws) return false;
        return root.windowList.some(w => w.workspaceId === ws.id && w.fullscreen
            && (w.output === "" || w.output === monitorName));
    }

    function monitorGeometry(screen) {
        const m = root.monitors.find(mm => mm.name === screen?.name);
        if (!m || !m.logical) return { x: 0, y: 0, scale: 1 };
        return { x: m.logical.x, y: m.logical.y, scale: m.logical.scale };
    }

    function updateAll() {
        getWindows.running = true;
        getWorkspaces.running = true;
        getOutputs.running = true;
    }

    Component.onCompleted: {
        updateAll();
        eventStream.running = true;
    }

    Process {
        id: getWindows
        command: ["hecate", "msg", "-j", "windows"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.windowList = JSON.parse(text).map(root.normalizeWindow);
                } catch (e) { console.log("[HecateBackend] windows parse error: " + e) }
            }
        }
    }

    Process {
        id: getWorkspaces
        command: ["hecate", "msg", "-j", "workspaces"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const raw = JSON.parse(text);
                    root.workspaces = raw;
                    let byId = {};
                    for (const ws of raw) byId[ws.id] = ws;
                    root.workspaceById = byId;
                    root.activeWorkspace = raw.find(ws => ws.is_focused) ?? raw.find(ws => ws.is_active) ?? null;
                } catch (e) { console.log("[HecateBackend] workspaces parse error: " + e) }
            }
        }
    }

    Process {
        id: getOutputs
        command: ["hecate", "msg", "-j", "outputs"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const raw = JSON.parse(text);
                    root.monitors = Object.keys(raw).map(name => {
                        const o = raw[name];
                        return {
                            name: name,
                            make: o.make ?? "",
                            model: o.model ?? "",
                            current_mode: o.current_mode,
                            modes: o.modes,
                            logical: o.logical,
                            focused: o.is_focused ?? false
                        };
                    });
                    root.focusedMonitor = root.monitors.find(m => m.focused) ?? root.monitors[0] ?? null;
                } catch (e) { console.log("[HecateBackend] outputs parse error: " + e) }
            }
        }
    }

    Process {
        id: eventStream
        command: ["hecate", "msg", "-j", "event-stream"]
        stdout: SplitParser {
            splitMarker: "\n"
            onRead: (line) => { if (line.trim().length > 0) refreshDebounce.restart() }
        }
        onExited: restartTimer.restart()
    }
    Timer { id: restartTimer; interval: 1000; onTriggered: eventStream.running = true }
    Timer { id: refreshDebounce; interval: 80; onTriggered: root.updateAll() }
}
