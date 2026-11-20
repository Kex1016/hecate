pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs
import qs.services
import qs.modules.common.widgets.haiiro as H
import "LauncherMatch.js" as Match

// The launcher window (Launcher board): `hecate: ~/launch`, a 32px prompt,
// then the results in labelled groups (apps, files, settings, actions).
// A prefix turns the same window into the clipboard, emoji picker,
// calculator, translator, command runner, file search or Claude's `?`
// (Clipboard board: "same window, same rows").
H.HWindow {
    id: root

    // room the result list may take before it scrolls
    property int listMax: 520
    property int selected: 0
    readonly property var groups: LauncherSearch.groups
    readonly property var rows: LauncherSearch.rows
    readonly property string mode: LauncherSearch.mode
    readonly property bool gridMode: mode === "emoji"
    readonly property int gridColumns: 10
    readonly property var current: rows[selected] ?? null
    readonly property bool quiet: mode === "clipboard" || mode === "emoji"
    signal done()

    readonly property var look: ({
        search: { label: "hecate: ~/launch", path: "~", command: "" },
        run: { label: "hecate: ~/run", path: "~", command: "sh -c" },
        calc: { label: "hecate: ~/calc", path: "~", command: "qalc" },
        clipboard: { label: "hecate: ~/clipboard", path: "~/clipboard", command: "grep" },
        emoji: { label: "hecate: ~/emoji", path: "~/emoji", command: "grep" },
        translate: { label: "hecate: ~/translate", path: "~", command: "trans" },
        files: { label: "hecate: ~/files", path: "~", command: "find" },
        claude: { label: "claude: ~/ask", path: "~", command: "claude" }
    })[mode] ?? { label: "hecate: ~/launch", path: "~", command: "" }

    label: look.label
    width: 720
    body.spacing: 10

    hints: {
        switch (root.mode) {
        case "clipboard":
            return [{ key: "enter", text: "paste" }, { key: "1-9", text: "pick" }, { key: "ctrl p", text: "pin" }, { key: "del", text: "forget" }, { key: "esc", text: "close" }];
        case "emoji":
            return [{ key: "←↑↓→", text: "move" }, { key: "enter", text: "type it" }, { key: "shift enter", text: "copy" }];
        case "search":
            return [{ key: "↑↓", text: "move" }, { key: "enter", text: "open" }, { key: "tab", text: "next group" }, { key: "esc", text: "close" }];
        default:
            return (root.current && !root.current.placeholder ? [{ key: "enter", text: root.current.hint ?? "go" }] : []).concat([{ key: "backspace", text: "leave " + LauncherSearch.parsed.prefix.trim() }, { key: "esc", text: "close" }]);
        }
    }

    function focusField() {
        prompt.field.forceActiveFocus();
    }

    // The field holds what follows the prefix. Typing a prefix at the start
    // of a plain search switches the mode and drops the prefix from the field.
    function syncFromQuery() {
        if (prompt.text !== LauncherSearch.text)
            prompt.text = LauncherSearch.text;
    }

    Connections {
        target: LauncherSearch
        function onQueryChanged() {
            root.syncFromQuery();
            root.selected = 0;
        }
    }
    Component.onCompleted: syncFromQuery()

    function groupStart(gi) {
        return root.rows.findIndex(r => r.groupIndex === gi);
    }

    function move(delta) {
        if (root.rows.length === 0)
            return;
        root.selected = Math.max(0, Math.min(root.rows.length - 1, root.selected + delta));
    }

    function nextGroup(dir) {
        if (!root.current)
            return;
        const n = root.groups.length;
        if (n < 2)
            return;
        const gi = (root.current.groupIndex + dir + n) % n;
        root.selected = Math.max(0, root.groupStart(gi));
    }

    function activate(index, alternate) {
        const row = root.rows[index];
        if (!row)
            return;
        if (row.placeholder)
            return;
        if (alternate && row.copy)
            row.copy();
        else
            row.execute();
        root.done();
    }

    // n-th unpinned clipboard entry, 1-based
    function clipNumber(index) {
        let n = 0;
        for (let i = 0; i <= index && i < root.rows.length; i++)
            if (!root.rows[i].pinned)
                n++;
        return root.rows[index]?.pinned ? -1 : n;
    }

    function ensureVisible(item) {
        const y = item.mapToItem(list.contentItem, 0, 0).y;
        if (y < list.contentY)
            list.contentY = y;
        else if (y + item.height > list.contentY + list.height)
            list.contentY = y + item.height - list.height;
    }

    LauncherPrompt {
        id: prompt
        Layout.fillWidth: true
        path: root.look.path
        command: root.look.command
        role: root.quiet ? "body" : "nav"
        field.focus: true

        onEdited: LauncherSearch.query = LauncherSearch.parsed.prefix + prompt.text

        onKeyPressed: event => {
            const ctrl = event.modifiers & Qt.ControlModifier;
            const shift = event.modifiers & Qt.ShiftModifier;
            switch (event.key) {
            case Qt.Key_Escape:
                GlobalStates.overviewOpen = false;
                break;
            case Qt.Key_Down:
                root.move(root.gridMode ? root.gridColumns : 1);
                break;
            case Qt.Key_Up:
                root.move(root.gridMode ? -root.gridColumns : -1);
                break;
            case Qt.Key_Left:
            case Qt.Key_Right:
                if (!root.gridMode)
                    return;
                root.move(event.key === Qt.Key_Left ? -1 : 1);
                break;
            case Qt.Key_PageDown:
                root.move(8);
                break;
            case Qt.Key_PageUp:
                root.move(-8);
                break;
            case Qt.Key_Tab:
                root.nextGroup(1);
                break;
            case Qt.Key_Backtab:
                root.nextGroup(-1);
                break;
            case Qt.Key_Return:
            case Qt.Key_Enter:
                root.activate(root.selected, shift);
                break;
            case Qt.Key_Backspace:
                // backspace on an empty field leaves the mode
                if (prompt.text.length > 0 || root.mode === "search")
                    return;
                LauncherSearch.query = "";
                break;
            case Qt.Key_Delete:
                if (root.mode !== "clipboard" || !root.current?.clip)
                    return;
                Cliphist.deleteEntry(root.current.clip);
                break;
            case Qt.Key_P:
                if (!ctrl || root.mode !== "clipboard" || !root.current?.clip)
                    return;
                Cliphist.togglePin(root.current.clip);
                break;
            default:
                // 1-9 picks a clipboard entry while nothing is typed
                if (root.mode === "clipboard" && prompt.text.length === 0 && event.key >= Qt.Key_1 && event.key <= Qt.Key_9) {
                    const want = event.key - Qt.Key_0;
                    for (let i = 0; i < root.rows.length; i++) {
                        if (root.clipNumber(i) === want) {
                            root.activate(i, false);
                            break;
                        }
                    }
                    break;
                }
                return;
            }
            event.accepted = true;
        }
    }

    H.HRule {
        Layout.fillWidth: true
        visible: root.rows.length > 0 || root.mode !== "search" || LauncherSearch.text.length > 0
    }

    Flickable {
        id: list
        Layout.fillWidth: true
        Layout.preferredHeight: Math.min(root.listMax, groupsColumn.implicitHeight)
        // not from the column's height: a hidden parent hides the rows it is measured from
        visible: root.rows.length > 0
        contentHeight: groupsColumn.implicitHeight
        contentWidth: width
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height

        Column {
            id: groupsColumn
            width: list.width
            spacing: 10

            Repeater {
                model: root.groups

                Column {
                    id: group
                    required property var modelData
                    required property int index
                    readonly property int start: root.groupStart(index)
                    width: groupsColumn.width
                    spacing: 10

                    // a mode with one list (clipboard, run, files) needs no heading
                    H.HLabel {
                        visible: root.mode === "search" || group.modelData.grid === true || root.groups.length > 1
                        text: group.modelData.name
                    }

                    // list groups
                    Column {
                        visible: !group.modelData.grid
                        width: parent.width
                        Repeater {
                            model: group.modelData.grid ? [] : group.modelData.rows
                            Loader {
                                id: rowLoader
                                required property var modelData
                                required property int index
                                readonly property int flat: group.start + index
                                readonly property bool lead: root.mode === "clipboard" || (modelData.verb ?? "").length > 0
                                width: parent?.width ?? 0
                                sourceComponent: lead ? leadRow : plainRow

                                Component {
                                    id: plainRow
                                    LauncherRow {
                                        entry: rowLoader.modelData
                                        selected: root.selected === rowLoader.flat
                                        onEntered: root.selected = rowLoader.flat
                                        onClicked: root.activate(rowLoader.flat, false)
                                        onSelectedChanged: if (selected) root.ensureVisible(rowLoader)
                                    }
                                }
                                Component {
                                    id: leadRow
                                    LauncherLeadRow {
                                        entry: rowLoader.modelData
                                        number: root.mode === "clipboard" ? root.clipNumber(rowLoader.flat) : -1
                                        selected: root.selected === rowLoader.flat
                                        onEntered: root.selected = rowLoader.flat
                                        onClicked: root.activate(rowLoader.flat, false)
                                        onSelectedChanged: if (selected) root.ensureVisible(rowLoader)
                                    }
                                }
                            }
                        }
                    }

                    // emoji and symbol grids, ten to a row
                    Grid {
                        id: grid
                        visible: group.modelData.grid === true
                        width: parent.width
                        columns: root.gridColumns
                        columnSpacing: 4
                        rowSpacing: 4
                        readonly property int cell: Math.floor((width - (columns - 1) * columnSpacing) / columns)

                        Repeater {
                            model: group.modelData.grid ? group.modelData.rows : []
                            Rectangle {
                                id: cellItem
                                required property var modelData
                                required property int index
                                readonly property int flat: group.start + index
                                readonly property bool isSelected: root.selected === flat
                                width: grid.cell
                                height: glyph.implicitHeight + 12
                                color: isSelected ? H.HTokens.overlay : "transparent"
                                border.width: isSelected ? 1 : 0
                                border.color: H.HTokens.rose
                                onIsSelectedChanged: if (isSelected) root.ensureVisible(cellItem)

                                H.HText {
                                    id: glyph
                                    anchors.centerIn: parent
                                    text: cellItem.modelData.text
                                    font.pixelSize: group.modelData.name === "symbols" ? H.HTokens.fontBody : 24
                                    lineHeight: H.HTokens.lineBox(font.pixelSize)
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    onEntered: root.selected = cellItem.flat
                                    onClicked: root.activate(cellItem.flat, false)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // the selected emoji's name, under the grids
    H.HText {
        visible: root.gridMode && root.current !== null
        text: root.current ? root.current.meta : ""
        role: "caption"
        ink: "muted"
    }

    // nothing to show yet
    H.HText {
        readonly property string empty: {
            const typed = LauncherSearch.text.trim().length > 0;
            switch (root.mode) {
            case "clipboard": return typed ? "nothing copied matches" : "nothing copied yet";
            case "emoji": return typed ? "no emoji or symbol matches" : "";
            case "files": return typed ? (LauncherSearch.filesFor === LauncherSearch.text.trim() ? "no files under ~ match" : "searching ~…") : "type part of a file name";
            case "run": return typed ? "" : "type a command";
            case "calc": return typed ? "" : "type a sum: 2^10, 10 kg to lb, 30 eur in huf";
            case "translate": return typed ? "" : "type text to translate";
            default: return "";
            }
        }
        visible: root.rows.length === 0 && empty.length > 0
        text: empty
        role: "caption"
        ink: "inkMuted"
    }

    // `?` with nothing listening yet
    H.HText {
        visible: root.mode === "claude" && !LauncherSearch.claudeReady
        Layout.fillWidth: true
        wrapMode: Text.WordWrap
        lineHeightMode: Text.FixedHeight
        text: "the ask panel arrives with the Claude tools. nothing is sent anywhere."
        role: "caption"
        ink: "inkMuted"
    }
}
