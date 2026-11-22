import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs
import qs.services
import qs.modules.common.widgets.haiiro as H

// `cat ~/notes/scratch.md`: the same plain text file as the window, on ruled
// lines with a block cursor and no frame: it looks like the file it edits.
// Saved as you type, a moment after you stop.
H.FreeWidget {
    id: root

    property bool loaded: false
    property string fileText: ""

    label: "cat ~/notes/scratch.md"

    Component.onCompleted: Quickshell.execDetached(["mkdir", "-p", DesktopWidgets.notePath.replace(/\/[^/]*$/, "")])
    Component.onDestruction: {
        if (saveTimer.running) { saveTimer.stop(); file.setText(note.currentText); }
        if (note.editing) GlobalStates.desktopWidgetKeyboardFocus = false;
    }

    FileView {
        id: file
        path: DesktopWidgets.notePath
        onLoaded: {
            if (!note.editing) root.fileText = file.text();
            root.loaded = true;
        }
        onLoadFailed: root.loaded = true
    }
    Timer {
        id: saveTimer
        interval: 600
        onTriggered: file.setText(note.currentText)
    }

    H.RuledNote {
        id: note
        Layout.preferredWidth: 260
        text: root.fileText
        editable: true
        endCursor: true
        onEdited: if (root.loaded) saveTimer.restart()
        onEditingChanged: {
            GlobalStates.desktopWidgetKeyboardFocus = editing;
            if (!editing && saveTimer.running) { saveTimer.stop(); file.setText(note.currentText); }
        }
    }
}
