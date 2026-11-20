import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs
import qs.services
import qs.modules.common.widgets.haiiro as H

// `cat ~/notes/scratch.md`: one plain text file, edited in place. Saved
// as you type, a moment after you stop.
H.HWindow {
    id: root

    property bool loaded: false

    widget: true
    label: "cat ~/notes/scratch.md"
    implicitWidth: 360

    Component.onCompleted: Quickshell.execDetached(["mkdir", "-p", DesktopWidgets.notePath.replace(/\/[^/]*$/, "")])

    FileView {
        id: file
        path: DesktopWidgets.notePath
        onLoaded: {
            if (!edit.activeFocus) edit.text = file.text();
            root.loaded = true;
        }
        onLoadFailed: root.loaded = true
    }
    Timer {
        id: saveTimer
        interval: 600
        onTriggered: file.setText(edit.text)
    }

    TextEdit {
        id: edit
        Layout.fillWidth: true
        Layout.preferredHeight: Math.max(contentHeight, 4 * font.pixelSize * H.HTokens.lineHeight)
        wrapMode: TextEdit.Wrap
        color: H.HTokens.text
        font.family: H.HTokens.mono
        font.pixelSize: H.HTokens.fontHeadingSm
        selectionColor: H.HTokens.pine
        selectedTextColor: H.HTokens.text
        renderType: Text.NativeRendering
        onTextChanged: if (root.loaded && activeFocus) saveTimer.restart()
        Keys.onEscapePressed: focus = false
        onActiveFocusChanged: {
            GlobalStates.desktopWidgetKeyboardFocus = activeFocus;
            if (!activeFocus && saveTimer.running) { saveTimer.stop(); file.setText(edit.text); }
        }
        Component.onDestruction: if (activeFocus) GlobalStates.desktopWidgetKeyboardFocus = false
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.IBeamCursor
            acceptedButtons: Qt.NoButton
        }
    }
    MouseArea {
        // pressing anywhere in the note puts the cursor in it
        parent: edit
        anchors.fill: parent
        z: -1
        onPressed: mouse => {
            GlobalStates.desktopWidgetKeyboardFocus = true;
            edit.forceActiveFocus();
            edit.cursorPosition = edit.positionAt(mouse.x, mouse.y);
        }
    }
}
