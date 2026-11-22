pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// Lines of text over dashed hairlines, with a block cursor where you type: a
// text file, not a box. One rule per line of the file; a line too long for
// the width wraps above its rule.
//   RuledNote { width: 260; text: file.text; editable: true; onEdited: text => file.save(text) }
// `text` goes in; what was typed comes out through `edited(text)` and
// `currentText`, and `text` is never written. `minLines` keeps empty ruled
// lines under a short note. `endCursor` shows the cursor after the last
// character while the note is not being edited.
Item {
    id: root

    property string text: ""
    property bool editable: false
    property int minLines: 4
    property bool endCursor: false
    property string role: "body"
    readonly property string currentText: lines.join("\n")
    readonly property bool editing: focusLine >= 0
    signal edited(string text)

    property var lines: [""]
    property int focusLine: -1
    readonly property int size: HTokens.role(role).size
    readonly property int rows: Math.max(minLines, lines.length)
    // what the line box adds to the glyph row
    readonly property int lead: HTokens.lineBox(size) - size

    implicitWidth: 260
    implicitHeight: column.implicitHeight

    onTextChanged: if (text !== currentText) lines = text.split("\n")
    Component.onCompleted: lines = text.split("\n")

    function commit(next, line, position) {
        lines = next;
        if (line !== undefined) Qt.callLater(root.place, line, position);
        root.edited(root.currentText);
    }
    function place(line, position) {
        const item = repeater.itemAt(Math.max(0, Math.min(line, lines.length - 1))) as Line;
        if (!item) return;
        item.edit.forceActiveFocus();
        item.edit.cursorPosition = Math.max(0, Math.min(position, item.edit.length));
    }
    function forceEditFocus() { if (editable) place(lines.length - 1, 1e9); }

    component Line: Item {
        id: line
        required property int index
        readonly property bool real: index < root.lines.length
        readonly property alias edit: edit
        width: column.width
        // .ruled > p: 4px above, 2px below and a 1px rule around a line box
        height: 4 + edit.contentHeight + root.lead + 2 + 1

        TextEdit {
            id: edit
            y: 4 + root.lead / 2
            width: parent.width
            text: line.real ? root.lines[line.index] : ""
            readOnly: !root.editable
            enabled: root.editable
            activeFocusOnTab: root.editable && line.index === 0
            wrapMode: TextEdit.Wrap
            textFormat: TextEdit.PlainText
            color: "transparent"
            selectionColor: HTokens.pine
            selectedTextColor: "transparent"
            renderType: Text.NativeRendering
            font.family: HTokens.mono
            font.pixelSize: root.size
            font.kerning: false
            font.hintingPreference: Font.PreferFullHinting
            cursorDelegate: BlockCursor {
                size: root.size
                visible: edit.activeFocus
            }

            onActiveFocusChanged: {
                if (activeFocus) root.focusLine = line.index;
                else if (root.focusLine === line.index) root.focusLine = -1;
            }
            // typing: the line's text, or several lines when a paste brought newlines
            onTextChanged: {
                if (!activeFocus) return;
                const next = root.lines.slice();
                while (next.length <= line.index) next.push("");
                if (next[line.index] === text) return;
                const parts = text.split("\n");
                next.splice(line.index, 1, ...parts);
                if (parts.length > 1) root.commit(next, line.index + parts.length - 1, parts[parts.length - 1].length);
                else root.commit(next);
            }
            Keys.onPressed: event => {
                const at = cursorPosition, here = line.index;
                const next = root.lines.slice();
                while (next.length <= here) next.push("");
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    next.splice(here, 1, text.slice(0, at), text.slice(at));
                    root.commit(next, here + 1, 0);
                } else if (event.key === Qt.Key_Backspace && at === 0 && selectedText.length === 0 && here > 0) {
                    const before = next[here - 1];
                    next.splice(here - 1, 2, before + text);
                    root.commit(next, here - 1, before.length);
                } else if (event.key === Qt.Key_Delete && at === length && selectedText.length === 0 && here < next.length - 1) {
                    next.splice(here, 2, text + next[here + 1]);
                    root.commit(next, here, at);
                } else if (event.key === Qt.Key_Up && here > 0 && cursorRectangle.y < 1) {
                    root.place(here - 1, 1e9);
                } else if (event.key === Qt.Key_Down && here < root.rows - 1 && cursorRectangle.y + cursorRectangle.height > contentHeight - 1) {
                    if (here + 1 >= root.lines.length) { next.push(""); root.commit(next, here + 1, 0); }
                    else root.place(here + 1, Math.min(at, root.lines[here + 1].length));
                } else if (event.key === Qt.Key_Escape) {
                    focus = false;
                } else {
                    return;
                }
                event.accepted = true;
            }
        }
        // what you read: the same text with the free shadow
        FreeText {
            y: 4 + root.lead / 2
            width: parent.width
            text: edit.text
            role: root.role
            wrapMode: Text.Wrap
            lineHeight: Math.max(1, edit.contentHeight / Math.max(1, edit.lineCount))
            verticalAlignment: Text.AlignTop
        }
        BlockCursor {
            readonly property rect end: edit.positionToRectangle(edit.length)
            visible: root.endCursor && !root.editing && line.index === root.lines.length - 1
            size: root.size
            x: end.x
            y: 4 + root.lead / 2 + end.y
        }
        PixelChecker {
            y: parent.height - 1
            width: parent.width
            height: dev(1) / dpr
            cell: 3
            color: HTokens.inkHairline
        }
        MouseArea {
            anchors.fill: parent
            enabled: root.editable && !edit.activeFocus
            cursorShape: Qt.IBeamCursor
            onClicked: mouse => {
                if (!line.real) root.place(root.lines.length - 1, 1e9);
                else root.place(line.index, edit.positionAt(mouse.x, mouse.y - 4 - root.lead / 2));
            }
        }
    }

    Column {
        id: column
        width: root.width

        Repeater {
            id: repeater
            model: root.rows

            Line {}
        }
    }
}
