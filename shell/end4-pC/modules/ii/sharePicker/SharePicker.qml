pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Widgets
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets

/**
 * hecate: the screen sharing picker. xdg-desktop-portal-wlr runs `hecate-share-picker`, which
 * calls `sharePicker.pick(<fifo>)` and prints what we write into the fifo: "Monitor: <output>",
 * "Window: <ext-foreign-toplevel-list identifier>", or nothing to decline.
 */
Scope {
    id: root

    property string answerPath: ""
    readonly property bool open: answerPath !== ""
    property string category: "screens"
    property string selected: ""
    // Read from the start: ToplevelManager fills its list asynchronously on first use.
    readonly property var toplevels: ToplevelManager.toplevels.values

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

    // The Quickshell toplevel (for the preview) of a hecate window: same app id and title,
    // the n-th of those in creation order on both sides.
    function toplevelFor(win) {
        const same = WM.windowList
            .filter(w => w.appId === win.appId && w.title === win.title)
            .sort((a, b) => Number(a.id) - Number(b.id));
        const k = same.findIndex(w => w.id === win.id);
        const tops = root.toplevels.filter(t => t.appId === win.appId && t.title === win.title);
        return tops[k] ?? null;
    }

    Process { id: answerProc }

    IpcHandler {
        target: "sharePicker"

        function pick(answerPath: string): void {
            root.finish(""); // A request still open is declined.
            root.selected = "";
            root.category = "screens";
            root.answerPath = answerPath;
        }
    }

    Loader {
        active: root.open
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

            WindowDialog {
                id: dialog
                anchors.fill: parent
                backgroundWidth: Math.min(940, panel.width - 80)
                show: false
                focus: true
                Component.onCompleted: show = true
                onDismiss: root.finish("")
                Keys.onReturnPressed: if (root.selected !== "") root.finish(root.selected)

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    MaterialSymbol {
                        iconSize: 26
                        text: "screen_share"
                        color: Appearance.colors.colSecondary
                    }
                    WindowDialogTitle {
                        Layout.fillWidth: true
                        text: Translation.tr("Share your screen")
                    }
                    // M3 Expressive connected button group.
                    Row {
                        spacing: 2
                        SelectionGroupButton {
                            leftmost: true
                            buttonIcon: "monitor"
                            buttonText: Translation.tr("Screens")
                            toggled: root.category === "screens"
                            onClicked: root.category = "screens"
                        }
                        SelectionGroupButton {
                            rightmost: true
                            buttonIcon: "select_window"
                            buttonText: Translation.tr("Windows")
                            toggled: root.category === "windows"
                            onClicked: root.category = "windows"
                        }
                    }
                }

                GridView {
                    id: grid
                    Layout.fillWidth: true
                    // Fixed: WindowDialog sizes its card once, when it opens.
                    Layout.preferredHeight: cellHeight * 2
                    readonly property int columns: Math.max(1, Math.floor(width / 300))
                    cellWidth: width / columns
                    cellHeight: 210
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds
                    model: root.category === "screens"
                        ? Quickshell.screens.map(s => ({ answer: `Monitor: ${s.name}`, label: s.name, sub: `${s.width}×${s.height}`, screen: s, win: null }))
                        : WM.windowList.map(w => ({ answer: `Window: ${root.windowIdentifier(w.id)}`, label: w.title || w.appId, sub: w.appId, screen: null, win: w }))

                    delegate: Item {
                        id: cell
                        required property var modelData
                        readonly property bool selected: root.selected === modelData.answer
                        width: grid.cellWidth
                        height: grid.cellHeight

                        Rectangle {
                            id: card
                            anchors.fill: parent
                            anchors.margins: 6
                            radius: Appearance.rounding.normal
                            color: cell.selected ? Appearance.colors.colSecondaryContainer
                                : mouse.containsMouse ? Appearance.colors.colLayer2Hover : Appearance.colors.colLayer2
                            border.width: cell.selected ? 2 : 0
                            border.color: Appearance.colors.colPrimary
                            Behavior on color {
                                animation: Appearance.animation.elementMoveFast.colorAnimation.createObject(this)
                            }

                            ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: 10
                                spacing: 8

                                Rectangle {
                                    Layout.fillWidth: true
                                    Layout.fillHeight: true
                                    radius: Appearance.rounding.small
                                    color: Appearance.colors.colLayer1
                                    clip: true

                                    ScreencopyView {
                                        id: preview
                                        anchors.fill: parent
                                        anchors.margins: 2
                                        live: true
                                        captureSource: cell.modelData.screen ?? root.toplevelFor(cell.modelData.win)
                                    }
                                    MaterialSymbol {
                                        anchors.centerIn: parent
                                        visible: !preview.hasContent
                                        iconSize: 40
                                        text: cell.modelData.screen ? "monitor" : "select_window"
                                        color: Appearance.colors.colSubtext
                                    }
                                }

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 8
                                    IconImage {
                                        visible: cell.modelData.win !== null
                                        implicitSize: 20
                                        source: cell.modelData.win ? SystemAppearance.iconPath(AppSearch.guessIcon(cell.modelData.win.appId), "image-missing") : ""
                                    }
                                    MaterialSymbol {
                                        visible: cell.modelData.screen !== null
                                        iconSize: 20
                                        text: "monitor"
                                        color: Appearance.colors.colOnLayer2
                                    }
                                    StyledText {
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                        text: cell.modelData.label
                                        color: cell.selected ? Appearance.colors.colOnSecondaryContainer : Appearance.colors.colOnLayer2
                                    }
                                    StyledText {
                                        elide: Text.ElideRight
                                        Layout.maximumWidth: 110
                                        text: cell.modelData.sub
                                        font.pixelSize: Appearance.font.pixelSize.smaller
                                        color: Appearance.colors.colSubtext
                                    }
                                }
                            }

                            MouseArea {
                                id: mouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.selected = cell.modelData.answer
                            }
                        }
                    }
                }

                StyledText {
                    Layout.fillWidth: true
                    visible: grid.count === 0
                    horizontalAlignment: Text.AlignHCenter
                    color: Appearance.colors.colSubtext
                    text: Translation.tr("No windows to share")
                }

                WindowDialogButtonRow {
                    Layout.bottomMargin: 10
                    Item {
                        Layout.fillWidth: true
                    }
                    DialogButton {
                        buttonText: Translation.tr("Cancel")
                        onClicked: root.finish("")
                    }
                    DialogButton {
                        buttonText: Translation.tr("Share")
                        enabled: root.selected !== ""
                        onClicked: root.finish(root.selected)
                    }
                }
            }
        }
    }
}
