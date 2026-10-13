pragma Singleton
pragma ComponentBehavior: Bound

import qs.modules.common
import qs.modules.common.functions
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland

/**
 * A service that provides access to Hyprland keybinds.
 * Uses the `get_keybinds.py` script to parse comments in config files in a certain format and convert to JSON.
 */
Singleton {
    id: root
    property string keybindParserPath: FileUtils.trimFileProtocol(`${Directories.scriptPath}/hyprland/get_keybinds.py`)
    property string defaultKeybindConfigPath: FileUtils.trimFileProtocol(`${Directories.config}/hypr/hyprland/keybinds.lua`)
    property string userKeybindConfigPath: FileUtils.trimFileProtocol(`${Directories.config}/hypr/custom/keybinds.lua`)
    property var defaultKeybinds: {"children": []}
    property var userKeybinds: {"children": []}
    property var keybinds: ({
        children: [
            ...(defaultKeybinds.children ?? []),
            ...(userKeybinds.children ?? []),
        ]
    })

    // hecate: binds come from `hecate msg -j binds`, converted to the parser's section format.
    readonly property bool onHecate: WM.compositor === "hecate"

    function describeAction(action) {
        if (typeof action === "string") return action.replace(/_/g, " ").replace(/^./, c => c.toUpperCase());
        const name = Object.keys(action ?? {})[0] ?? "";
        const arg = action?.[name];
        if (name === "spawn") return "Run " + (Array.isArray(arg) ? arg.join(" ") : arg);
        if (name === "spawn_sh") return "Run " + arg;
        if (["on_press", "on_release", "tap", "action"].includes(name)) return describeAction(arg);
        return describeAction(name) + (arg !== undefined && typeof arg !== "object" ? " " + arg : "");
    }

    function hecateSections(binds) {
        const titles = { base: "hecate", shell: "Shell", config: "Your config" };
        const sections = {};
        for (const b of binds) {
            const parts = b.key.split("+");
            const entry = {
                // `Mod` is Super on a real session.
                mods: parts.slice(0, -1).map(m => m.toUpperCase() === "MOD" ? "SUPER" : m.toUpperCase()),
                key: parts[parts.length - 1],
                dispatcher: "",
                params: "",
                comment: b.description ?? root.describeAction(b.action),
            };
            const name = titles[b.source] ?? b.source;
            (sections[name] = sections[name] ?? { children: [], keybinds: [], name: name }).keybinds.push(entry);
        }
        return { children: Object.values(sections) };
    }

    Process {
        id: getHecateBinds
        running: root.onHecate
        command: ["hecate", "msg", "-j", "binds"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.defaultKeybinds = root.hecateSections(JSON.parse(text));
                    root.userKeybinds = { children: [] };
                } catch (e) {
                    console.error("[CheatsheetKeybinds] Error reading hecate binds:", e)
                }
            }
        }
    }

    Process {
        id: hecateEvents
        running: root.onHecate
        command: ["hecate", "msg", "-j", "event-stream"]
        stdout: SplitParser {
            onRead: line => { if (line.indexOf("ConfigReloaded") !== -1) getHecateBinds.running = true }
        }
    }

    Connections {
        target: Hyprland
        enabled: !root.onHecate

        function onRawEvent(event) {
            if (event.name == "configreloaded") {
                getDefaultKeybinds.running = true
                getUserKeybinds.running = true
            }
        }
    }

    Process {
        id: getDefaultKeybinds
        running: !root.onHecate
        command: [root.keybindParserPath, "--path", root.defaultKeybindConfigPath]
        
        stdout: SplitParser {
            onRead: data => {
                try {
                    root.defaultKeybinds = JSON.parse(data)
                } catch (e) {
                    console.error("[CheatsheetKeybinds] Error parsing keybinds:", e)
                }
            }
        }
    }

    Process {
        id: getUserKeybinds
        running: !root.onHecate
        command: [root.keybindParserPath, "--path", root.userKeybindConfigPath]
        
        stdout: SplitParser {
            onRead: data => {
                try {
                    root.userKeybinds = JSON.parse(data)
                } catch (e) {
                    console.error("[CheatsheetKeybinds] Error parsing keybinds:", e)
                }
            }
        }
    }
}

