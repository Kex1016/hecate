pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

/**
 * hecate: the screen sharing picker. xdg-desktop-portal-wlr runs `hecate-share-picker`, which
 * calls `sharePicker.pick(<fifo>)` and prints what we write into the fifo: "Monitor: <output>",
 * "Window: <ext-foreign-toplevel-list identifier>", or nothing to decline.
 *
 * Screens and windows sit on one sheet under a pair of tabs. Arrow keys move the choice, tab
 * switches between screens and windows, enter shares, esc cancels.
 */
Scope {
    id: root

    property string answerPath: ""
    // who asks, for the window's label; set by `pickFor`, empty from the plain `pick`
    property string requester: ""
    readonly property bool open: answerPath !== ""
    // mapped from the request until the close frames have played
    property bool mapped: false
    property string category: "screens"
    property string selected: ""

    readonly property var screenChoices: Quickshell.screens.map(s => ({
        answer: `Monitor: ${s.name}`,
        short: s.name,
        label: `${s.name} · ${s.width}×${s.height}` + (s.name === WM.focusedMonitor?.name ? " · main" : ""),
        screen: s,
        win: null
    }))
    readonly property var windowChoices: WM.windowList.map(w => ({
        answer: `Window: ${root.windowIdentifier(w.id)}`,
        short: w.title || w.appId,
        label: w.title ? `${w.appId}: ${w.title}` : w.appId,
        screen: null,
        win: w
    }))
    readonly property var current: root.category === "screens" ? root.screenChoices : root.windowChoices
    readonly property var chosen: root.screenChoices.concat(root.windowChoices).find(c => c.answer === root.selected) ?? null

    function finish(answer) {
        if (!root.open) return;
        // The picker script may already have given up (timeout): don't block on its fifo.
        answerProc.command = ["timeout", "5", "sh", "-c", 'printf "%s" "$1" > "$2"', "sh", answer, root.answerPath];
        answerProc.running = true;
        root.answerPath = "";
        root.selected = "";
    }

    // hecate's ext-foreign-toplevel-list identifier for a window id.
    function windowIdentifier(id) {
        return "hecate-" + Number(id).toString(16).padStart(16, "0");
    }

    function begin(answerPath, requester) {
        root.finish(""); // A request still open is declined.
        root.requester = requester;
        root.category = "screens";
        root.selected = root.screenChoices[0]?.answer ?? "";
        root.answerPath = answerPath;
        root.mapped = true;
    }

    function switchTo(category) {
        if (root.category === category) return;
        root.category = category;
        root.selected = root.current[0]?.answer ?? "";
    }

    // move the choice by `step` entries within the shown category
    function move(step) {
        const list = root.current;
        if (list.length === 0) return;
        const at = list.findIndex(c => c.answer === root.selected);
        root.selected = list[Math.max(0, Math.min(list.length - 1, at < 0 ? 0 : at + step))].answer;
    }

    Process { id: answerProc }

    IpcHandler {
        target: "sharePicker"

        function pick(answerPath: string): void {
            root.begin(answerPath, "");
        }

        function pickFor(answerPath: string, requester: string): void {
            root.begin(answerPath, requester);
        }
    }

    Loader {
        active: root.mapped
        sourceComponent: PanelWindow {
            id: panel
            screen: Quickshell.screens.find(s => s.name === WM.focusedMonitor?.name) ?? Quickshell.screens[0]
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "quickshell:sharePicker"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
            color: "transparent"
            anchors {
                top: true
                bottom: true
                left: true
                right: true
            }
            margins.top: H.HTokens.barHeight

            H.Veil {
                anchors.fill: parent
                heavy: true
                onClicked: root.finish("")
            }

            FocusScope {
                anchors.fill: parent
                focus: true
                Keys.onPressed: event => {
                    const columns = root.category === "screens" ? 2 : 3;
                    if (event.key === Qt.Key_Escape) root.finish("");
                    else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        if (root.selected !== "") root.finish(root.selected);
                    }
                    else if (event.key === Qt.Key_Left) root.move(-1);
                    else if (event.key === Qt.Key_Right) root.move(1);
                    else if (event.key === Qt.Key_Up) root.move(-columns);
                    else if (event.key === Qt.Key_Down) root.move(columns);
                    else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab)
                        root.switchTo(root.category === "screens" ? "windows" : "screens");
                    else return;
                    event.accepted = true;
                }
            }

            H.PixelTransition {
                anchors.centerIn: parent
                width: win.implicitWidth
                height: win.implicitHeight
                shown: root.open
                onClosed: if (!root.open) root.mapped = false

                H.HWindow {
                    id: win
                    width: Math.min(680, panel.width - 80)
                    height: implicitHeight
                    label: root.requester !== "" ? `share: ${root.requester} wants your screen` : "share: your screen"
                    hints: [
                        { key: "←↑↓→", text: "move" },
                        { key: "tab", text: "screens / windows" },
                        { key: "enter", text: "share" },
                        { key: "esc", text: "cancel" }
                    ]

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Row {
                            H.InkButton {
                                text: "screens"
                                ink: "c5"
                                flat: root.category !== "screens"
                                on: root.category === "screens"
                                activeFocusOnTab: false
                                onClicked: root.switchTo("screens")
                            }
                            H.InkButton {
                                text: "windows"
                                ink: "c5"
                                flat: root.category !== "windows"
                                on: root.category === "windows"
                                activeFocusOnTab: false
                                onClicked: root.switchTo("windows")
                            }
                        }

                        // Both sections stay on the sheet; a tall list scrolls inside it.
                        Flickable {
                            id: sheet
                            Layout.fillWidth: true
                            Layout.preferredHeight: Math.min(sections.implicitHeight, Math.max(240, panel.height - 420))
                            contentWidth: width
                            contentHeight: sections.implicitHeight
                            clip: true
                            boundsBehavior: Flickable.StopAtBounds

                            ColumnLayout {
                                id: sections
                                width: sheet.width
                                spacing: 12

                                GridLayout {
                                    Layout.fillWidth: true
                                    columns: 2
                                    uniformCellWidths: true
                                    columnSpacing: 12
                                    rowSpacing: 12
                                    Repeater {
                                        model: root.screenChoices
                                        delegate: Source {
                                            Layout.fillWidth: true
                                            Layout.preferredWidth: 1
                                            previewHeight: 110
                                            quiet: true
                                        }
                                    }
                                }

                                H.HLabel { text: "windows" }

                                GridLayout {
                                    Layout.fillWidth: true
                                    columns: 3
                                    uniformCellWidths: true
                                    columnSpacing: 12
                                    rowSpacing: 12
                                    Repeater {
                                        model: root.windowChoices
                                        delegate: Source {
                                            Layout.fillWidth: true
                                            Layout.preferredWidth: 1
                                            previewHeight: 80
                                        }
                                    }
                                }
                                H.HText {
                                    visible: root.windowChoices.length === 0
                                    text: "No windows to share."
                                    ink: "c1"
                                }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            Item { Layout.fillWidth: true }
                            H.InkButton { text: "cancel"; ink: "c1"; onClicked: root.finish("") }
                            H.InkButton {
                                text: root.chosen ? `share ${root.chosen.short}` : "share"
                                ink: "c3"
                                enabled: root.chosen !== null
                                onClicked: root.finish(root.selected)
                            }
                        }
                    }
                }
            }
        }
    }

    // One screen or window: a live capture in a frame, and what it is.
    component Source: Item {
        id: src
        required property var modelData
        property int previewHeight: 110
        // screens that are not the chosen one are written in muted ink
        property bool quiet: false
        readonly property bool on: root.selected === modelData.answer
        implicitHeight: col.implicitHeight + 12

        Rectangle {
            anchors.fill: parent
            visible: src.on
            color: H.HTokens.overlay
        }
        ColumnLayout {
            id: col
            x: 6
            y: 6
            width: parent.width - 12
            spacing: 6

            Item {
                id: frame
                Layout.fillWidth: true
                Layout.preferredHeight: src.previewHeight

                // the picture, inset by the frame so it never draws under it
                Item {
                    anchors.fill: parent
                    anchors.margins: src.on ? 5 : 2
                    clip: true
                    H.Dither {
                        anchors.fill: parent
                        tone: src.on ? "pine" : "muted"
                        big: src.on
                    }
                    ScreencopyView {
                        id: preview
                        anchors.fill: parent
                        live: true
                        captureSource: src.modelData.screen ?? WM.toplevelFor(src.modelData.win)
                    }
                    // the checker of the veil over the capture: the same two-tone dither as images
                    H.Dither {
                        anchors.fill: parent
                        visible: preview.hasContent
                        ink: H.HTokens.base
                        ground: "transparent"
                    }
                }
                // 2px muted frame, or the 5px double pine frame for the choice
                Rectangle {
                    anchors.fill: parent
                    visible: !src.on
                    color: "transparent"
                    border.width: 2
                    border.color: H.HTokens.highlightHigh
                }
                Repeater {
                    model: src.on ? [0, 3] : []
                    Rectangle {
                        required property int modelData
                        x: modelData
                        y: modelData
                        width: frame.width - 2 * modelData
                        height: frame.height - 2 * modelData
                        color: "transparent"
                        border.width: 2
                        border.color: H.HTokens.pine
                    }
                }
            }
            H.HText {
                Layout.fillWidth: true
                text: src.modelData.label
                role: "label"
                ink: src.quiet && !src.on ? "c1" : "text"
                elide: Text.ElideRight
                maximumLineCount: 1
            }
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: {
                root.category = src.modelData.screen ? "screens" : "windows";
                root.selected = src.modelData.answer;
            }
            onDoubleClicked: root.finish(src.modelData.answer)
        }
    }
}
