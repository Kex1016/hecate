pragma Singleton
pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io

/**
 * Global shortcuts for compositors without Hyprland's GlobalShortcut protocol (hecate).
 * CompositorGlobalShortcut registers here, and the compositor's binds fire shortcuts by name:
 *
 *   qs -c end4-pC ipc call shortcut press searchToggleRelease
 *   qs -c end4-pC ipc call shortcut release workspaceNumber
 */
Singleton {
    id: root

    // name -> [CompositorGlobalShortcut]
    property var shortcuts: ({})

    function register(shortcut) {
        if (!shortcut.name) return;
        const list = root.shortcuts[shortcut.name] ?? [];
        if (list.indexOf(shortcut) === -1) list.push(shortcut);
        root.shortcuts[shortcut.name] = list;
    }

    function unregister(shortcut) {
        const list = root.shortcuts[shortcut.name];
        if (!list) return;
        const i = list.indexOf(shortcut);
        if (i !== -1) list.splice(i, 1);
        if (list.length === 0) delete root.shortcuts[shortcut.name];
    }

    function fire(name, pressed) {
        const list = root.shortcuts[name];
        if (!list) {
            console.log("[ShortcutRegistry] no shortcut named " + name);
            return false;
        }
        for (const s of list) {
            if (pressed) s.pressed(); else s.released();
        }
        return true;
    }

    IpcHandler {
        target: "shortcut"

        function press(name: string): void { root.fire(name, true); }
        function release(name: string): void { root.fire(name, false); }
        // Press then release, for binds that act on press only.
        function trigger(name: string): void { root.fire(name, true); root.fire(name, false); }
        function list(): string { return Object.keys(root.shortcuts).sort().join("\n"); }
    }
}
