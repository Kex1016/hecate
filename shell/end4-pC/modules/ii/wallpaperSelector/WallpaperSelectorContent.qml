pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.modules.common
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.modules.common.widgets.haiiro as H

// The wallpaper picker, the Wallpapers board: one window with the
// background mode, the dither and per-monitor settings, and the pictures.
// Only reachable once wallpapers are turned on (Wallpapers.enabled).
Item {
    id: root

    property bool monitorIsFocused: true
    readonly property string target: GlobalStates.wallpaperSelectorTarget
    readonly property var screenNames: Quickshell.screens.map(s => s.name)
    readonly property string targetMonitor: target.startsWith("monitor:") ? target.slice(8) : (screenNames[0] ?? "")
    readonly property string currentPath: target === "lockWall"
        ? Config.options.background.lockWall
        : target.startsWith("monitor:")
            ? (Wallpapers.monitorWallpaper(targetMonitor) || Config.options.background.wallpaperPath)
            : Config.options.background.wallpaperPath

    implicitWidth: 1100
    implicitHeight: 680

    function close() {
        Wallpapers.stopPreview();
        GlobalStates.wallpaperSelectorOpen = false;
    }

    function pick(path) {
        if (!path) return;
        if (target.startsWith("monitor:")) {
            Wallpapers.setMonitorWallpaper(targetMonitor, path);
        } else if (target === "lockWall") {
            Config.options.background.lockWall = path;
        } else {
            Wallpapers.apply(path);
        }
        if (Config.options.wallpaperSelector.closeAfterSelection) root.close();
    }

    function openFolder() { Wallpapers.openFolder(); }

    Keys.onPressed: event => {
        const k = event.key;
        if (k === Qt.Key_Escape) root.close();
        else if (k === Qt.Key_Left) grid.moveSelection(-1);
        else if (k === Qt.Key_Right) grid.moveSelection(1);
        else if (k === Qt.Key_Up) grid.moveSelection(-grid.columns);
        else if (k === Qt.Key_Down) grid.moveSelection(grid.columns);
        else if (k === Qt.Key_Return || k === Qt.Key_Enter) grid.activateCurrent();
        else if (k === Qt.Key_R) grid.randomPick();
        else if (k === Qt.Key_O) root.openFolder();
        else return;
        event.accepted = true;
    }

    onVisibleChanged: if (visible) forceActiveFocus()
    Component.onCompleted: {
        Wallpapers.setDirectory(`${Directories.pictures}/Wallpapers`);
        forceActiveFocus();
    }

    H.HWindow {
        anchors.fill: parent
        label: "ls ~/pictures/wallpapers"
        hints: [
            { key: "←↑↓→", text: "move" },
            { key: "enter", text: "set" },
            { key: "r", text: "random" },
            { key: "o", text: "open folder" },
            { key: "esc", text: "close" }
        ]
        note: "Mod+Ctrl+T opens this · Mod+Ctrl+Alt+T picks at random"

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 14

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                H.HText { text: "background"; role: "label"; ink: "c1"; Layout.rightMargin: 4 }
                H.Segmented {
                    model: ["blobs", "wallpaper"]
                    currentIndex: Wallpapers.enabled ? 1 : 0
                    onActivated: i => Wallpapers.setMode(i === 1 ? "wallpaper" : "blobs")
                }
                Item { Layout.preferredWidth: 16 }
                H.Chip {
                    text: "dither to haiiro"
                    interactive: true
                    on: Wallpapers.dither
                    onClicked: Wallpapers.setDither(!Wallpapers.dither)
                }
                H.Chip {
                    text: "per monitor"
                    interactive: true
                    on: Wallpapers.perMonitor
                    onClicked: Wallpapers.setPerMonitor(!Wallpapers.perMonitor)
                }
                Item { Layout.fillWidth: true }
                H.HText {
                    visible: Wallpapers.perMonitor
                    text: "applies to"
                    role: "caption"
                    ink: "c1"
                }
                Repeater {
                    model: Wallpapers.perMonitor ? root.screenNames : []
                    H.Chip {
                        required property string modelData
                        required property int index
                        text: modelData
                        interactive: true
                        on: modelData === root.targetMonitor && root.target !== "lockWall"
                        onClicked: GlobalStates.wallpaperSelectorTarget = index === 0 ? "wallpaper" : `monitor:${modelData}`
                    }
                }
            }

            // an iCloud album as the source instead of the folder (it hides itself outside
            // wallpaper mode)
            Loader {
                Layout.preferredWidth: 360
                active: Wallpapers.enabled
                visible: active && status === Loader.Ready
                source: active ? Qt.resolvedUrl("../background/widgets/ApplePhotosWidget.qml") : ""
            }

            LocalWallpaperGrid {
                id: grid
                Layout.fillWidth: true
                Layout.fillHeight: true
                currentPath: root.currentPath
                dither: Wallpapers.dither
                onPicked: path => root.pick(path)
            }

            H.HText {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: "The colours never change with the picture. With dithering on, whatever you choose is pressed into two tones of the night sky; with it off, the picture shows as it is."
                ink: "c4"
            }
        }
    }
}
