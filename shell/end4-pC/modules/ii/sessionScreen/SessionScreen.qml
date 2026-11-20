pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Hyprland
import qs
import qs.services
import qs.modules.common
import qs.modules.common.functions
import qs.modules.common.widgets.haiiro as H

/**
 * The session menu: `./session.sh` in a pine window over the heavy veil. Choose with the arrow
 * keys and enter, or press the key shown beside the row. Esc or a click outside cancels.
 */
Scope {
    id: root

    property string focusedScreenName: WM.niriLike
        ? (WM.focusedMonitor?.name ?? "")
        : (Hyprland.focusedMonitor?.name ?? "")

    property var focusedScreen: Quickshell.screens.find(s => s.name === root.focusedScreenName)
        ?? Quickshell.screens[0]

    // Mapped from the moment the menu opens until its close frames have played.
    property bool mapped: false

    // `key` is the shortcut, `ink` the colour of the row's word, `warn` asks first when
    // something that would be interrupted is running, `run` does it.
    readonly property var actions: [
        { id: "lock", text: "lock", key: "l", ink: "text", warn: false, run: () => Session.lock() },
        { id: "sleep", text: "sleep", key: "s", ink: "text", warn: false, run: () => Session.suspend() },
        { id: "logout", text: "log out", key: "o", ink: "text", warn: true, run: () => Session.logout() },
        { id: "hibernate", text: "hibernate", key: "h", ink: "text", warn: false, run: () => Session.hibernate() },
        { rule: true },
        { id: "reboot", text: "reboot", key: "r", ink: "c3", warn: true, run: () => Session.reboot() },
        { id: "shutdown", text: "shut down", key: "p", ink: "c2", warn: true, run: () => Session.poweroff() },
        { id: "windows", text: "boot windows", key: "w", ink: "c7", warn: false, run: null },
        { id: "firmware", text: "firmware settings", key: "f", ink: "c1", warn: true, run: () => Session.rebootToFirmware() }
    ]
    readonly property var choices: actions.filter(a => !a.rule)

    Connections {
        target: GlobalStates
        function onSessionOpenChanged() {
            if (GlobalStates.sessionOpen) {
                SessionWarnings.refresh();
                root.mapped = true;
            } else if (H.HTokens.cut) {
                root.mapped = false;
            }
        }
        function onScreenLockedChanged() {
            if (GlobalStates.screenLocked)
                GlobalStates.sessionOpen = false;
        }
    }

    Loader {
        active: root.mapped

        sourceComponent: PanelWindow {
            id: sessionRoot
            screen: root.focusedScreen
            property int current: 0
            // the action waiting for a yes or no because something is running
            property var pending: null
            property bool bootWindowsOpen: false
            readonly property bool warned: SessionWarnings.packageManagerRunning || SessionWarnings.downloadRunning

            function hide() {
                GlobalStates.sessionOpen = false;
            }
            function choose(action) {
                if (action.id === "windows") {
                    sessionRoot.bootWindowsOpen = true;
                } else if (action.warn && sessionRoot.warned) {
                    sessionRoot.pending = action;
                } else {
                    action.run();
                    sessionRoot.hide();
                }
            }
            function confirm() {
                const action = sessionRoot.pending;
                sessionRoot.pending = null;
                action.run();
                sessionRoot.hide();
            }

            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "quickshell:session"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
            color: "transparent"
            anchors {
                top: true
                bottom: true
                left: true
                right: true
            }
            // the bar stays on screen; the veil starts under it
            margins.top: H.HTokens.barHeight

            H.Veil {
                anchors.fill: parent
                heavy: true
                onClicked: sessionRoot.hide()
            }

            FocusScope {
                id: keys
                anchors.fill: parent
                focus: !sessionRoot.bootWindowsOpen
                Keys.onPressed: event => {
                    if (sessionRoot.pending) {
                        if (event.key === Qt.Key_Y || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                            sessionRoot.confirm();
                        } else if (event.key === Qt.Key_N || event.key === Qt.Key_Escape) {
                            sessionRoot.pending = null;
                        } else {
                            return;
                        }
                        event.accepted = true;
                        return;
                    }
                    const n = root.choices.length;
                    if (event.key === Qt.Key_Escape) {
                        sessionRoot.hide();
                    } else if (event.key === Qt.Key_Down || event.key === Qt.Key_J || event.key === Qt.Key_Tab) {
                        sessionRoot.current = (sessionRoot.current + 1) % n;
                    } else if (event.key === Qt.Key_Up || event.key === Qt.Key_K || event.key === Qt.Key_Backtab) {
                        sessionRoot.current = (sessionRoot.current + n - 1) % n;
                    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        sessionRoot.choose(root.choices[sessionRoot.current]);
                    } else {
                        const hit = root.choices.findIndex(a => a.key === event.text.toLowerCase() && event.text !== "");
                        if (hit < 0) return;
                        sessionRoot.current = hit;
                        sessionRoot.choose(root.choices[hit]);
                    }
                    event.accepted = true;
                }
            }

            H.PixelTransition {
                id: menu
                anchors.centerIn: parent
                width: win.implicitWidth
                height: win.implicitHeight
                shown: GlobalStates.sessionOpen
                onClosed: if (!GlobalStates.sessionOpen) root.mapped = false

                H.HWindow {
                    id: win
                    width: 560
                    height: implicitHeight
                    label: "hecate: ~/session"
                    hints: [
                        { key: "↑↓", text: "move" },
                        { key: "enter", text: "do it" },
                        { key: "esc", text: "or click outside: cancel" }
                    ]

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 10
                        H.PromptLine {
                            path: "~"
                            script: "./session.sh"
                        }
                        Item { implicitHeight: H.HTokens.lineBox(H.HTokens.fontBody) }
                        H.HText {
                            Layout.fillWidth: true
                            text: "The computer hums. It seems to be giving you a choice."
                            ink: "rose"
                            wrapMode: Text.Wrap
                        }
                        Item { implicitHeight: H.HTokens.lineBox(H.HTokens.fontBody) }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0
                            Repeater {
                                model: root.actions
                                delegate: Item {
                                    id: slot
                                    required property var modelData
                                    // position among the choices, to compare with `current`
                                    readonly property int choice: root.choices.findIndex(a => a.id === modelData.id)
                                    Layout.fillWidth: true
                                    implicitHeight: modelData.rule ? rule.implicitHeight : row.implicitHeight
                                    H.HRule {
                                        id: rule
                                        visible: slot.modelData.rule === true
                                        width: parent.width
                                    }
                                    H.ListRow {
                                        id: row
                                        visible: slot.modelData.rule !== true
                                        width: parent.width
                                        text: slot.modelData.text ?? ""
                                        ink: slot.modelData.ink ?? "text"
                                        selected: sessionRoot.current === slot.choice
                                        onEntered: sessionRoot.current = slot.choice
                                        onClicked: sessionRoot.choose(slot.modelData)
                                        H.Keycap { key: slot.modelData.key ?? "" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Something is running that a reboot or log out would interrupt.
            H.PixelTransition {
                id: warning
                anchors {
                    horizontalCenter: parent.horizontalCenter
                    top: menu.bottom
                    topMargin: 30
                }
                width: warn.implicitWidth
                height: warn.implicitHeight
                shown: sessionRoot.pending !== null
                H.HWindow {
                    id: warn
                    width: 560
                    height: implicitHeight
                    label: "wait"
                    variant: "gold"
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        H.HText {
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                            text: SessionWarnings.packageManagerRunning
                                ? "Your package manager is running."
                                : "There might be a download in progress. Check your Downloads folder."
                        }
                        H.HText {
                            ink: "c1"
                            text: (sessionRoot.pending?.text ?? "") + " anyway?"
                        }
                        RowLayout {
                            spacing: 8
                            H.InkButton { text: "yes"; ink: "c3"; onClicked: sessionRoot.confirm() }
                            H.InkButton { text: "no"; ink: "c2"; onClicked: sessionRoot.pending = null }
                        }
                    }
                }
            }

            Loader {
                anchors.fill: parent
                z: 100
                active: sessionRoot.bootWindowsOpen
                focus: active
                sourceComponent: BootWindowsDialog {
                    focus: true
                    onClosed: {
                        sessionRoot.bootWindowsOpen = false;
                        keys.forceActiveFocus();
                    }
                }
            }
        }
    }

    IpcHandler {
        target: "session"

        function toggle(): void {
            GlobalStates.sessionOpen = !GlobalStates.sessionOpen;
        }

        function close(): void {
            GlobalStates.sessionOpen = false;
        }

        function open(): void {
            GlobalStates.sessionOpen = true;
        }
    }

    CompositorGlobalShortcut {
        name: "sessionToggle"
        description: "Toggles session screen on press"

        onPressed: GlobalStates.sessionOpen = !GlobalStates.sessionOpen
    }

    CompositorGlobalShortcut {
        name: "sessionOpen"
        description: "Opens session screen on press"

        onPressed: {
            GlobalStates.sessionOpen = true;
        }
    }

    CompositorGlobalShortcut {
        name: "sessionClose"
        description: "Closes session screen on press"

        onPressed: {
            GlobalStates.sessionOpen = false;
        }
    }
}
