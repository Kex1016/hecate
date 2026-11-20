pragma Singleton
pragma ComponentBehavior: Bound

import qs.modules.common
import qs.modules.common.functions
import QtQuick
import Quickshell
import Quickshell.Io

/**
 * The emoji the launcher's `:` lists, from assets/emoji.txt (Unicode's emoji-test.txt,
 * fully-qualified, no skin tones): one `<emoji> <name>` per line after `### DATA ###`.
 */
Singleton {
    id: root
    property string emojiScriptPath: Quickshell.shellPath("assets/emoji.txt")
	property string lineBeforeData: "### DATA ###"
    property list<var> list
    readonly property var preparedEntries: list.map(a => ({
        name: Fuzzy.prepare(`${a}`),
        entry: a
    }))
    function fuzzyQuery(search: string): var {
        return Fuzzy.go(search, preparedEntries, {
            all: true,
            key: "name"
        }).map(r => {
            return r.obj.entry
        });
    }

    function load() {
        emojiFileView.reload()
    }

    function updateEmojis(fileContent) {
        const lines = fileContent.split("\n")
        const dataIndex = lines.indexOf(root.lineBeforeData)
        if (dataIndex === -1) {
            console.warn("No data section found in emoji script file.")
            return
        }
        const emojis = lines.slice(dataIndex + 1).filter(line => line.trim() !== "")
        root.list = emojis.map(line => line.trim())
    }

    FileView { 
        id: emojiFileView
        path: Qt.resolvedUrl(root.emojiScriptPath)
        onLoadedChanged: {
            const fileContent = emojiFileView.text()
            root.updateEmojis(fileContent)
        }
    }
}
