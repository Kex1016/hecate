pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Widgets
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets

/**
 * hecate: the Alt+Tab switcher. Windows in most recently used order, with live previews.
 *
 * Driven by hecate's `signal` binds (nix/shell-binds.nix): Alt+Tab / Alt+Shift+Tab move the
 * selection (the first press opens it on the previous window), releasing Alt switches,
 * Alt+Escape cancels. The keyboard stays with the focused window throughout.
 */
Scope {
    id: root

    property bool open: false
    // Windows when it opened, most recently used first.
    property var entries: []
    property int index: 0
    // A quick Alt+Tab tap switches without flashing the switcher up.
    property bool shown: false

    function start(step) {
        const wins = WM.windowList.slice().sort((a, b) => b.focusSerial - a.focusSerial);
        if (wins.length === 0) return;
        root.entries = wins;
        root.index = step > 0 ? Math.min(1, wins.length - 1) : wins.length - 1;
        root.open = true;
        showDelay.restart();
    }
    function move(step) {
        if (!root.open) {
            root.start(step);
            return;
        }
        const n = root.entries.length;
        root.index = ((root.index + step) % n + n) % n;
    }
    function commit() {
        if (!root.open) return;
        const win = root.entries[root.index];
        root.close();
        if (win) WM.focusWindow(win.id);
    }
    function close() {
        root.open = false;
        root.shown = false;
        showDelay.stop();
        root.entries = [];
    }

    Timer {
        id: showDelay
        interval: 120
        onTriggered: root.shown = root.open
    }

    Connections {
        target: WM.backend
        ignoreUnknownSignals: true
        function onCompositorSignal(name) {
            switch (name) {
            case "alt-tab-next": root.move(1); break;
            case "alt-tab-prev": root.move(-1); break;
            case "alt-tab-commit": root.commit(); break;
            case "alt-tab-cancel": root.close(); break;
            }
        }
    }

    // A window that closes while the switcher is up drops out of it.
    Connections {
        target: WM
        function onWindowListChanged() {
            if (!root.open) return;
            const ids = WM.windowList.map(w => w.id);
            const kept = root.entries.filter(e => ids.includes(e.id));
            if (kept.length === root.entries.length) return;
            if (kept.length === 0) {
                root.close();
                return;
            }
            const current = root.entries[root.index]?.id;
            root.entries = kept;
            root.index = Math.max(0, kept.findIndex(e => e.id === current));
        }
    }

    Loader {
        active: root.shown
        sourceComponent: PanelWindow {
            id: panel
            screen: Quickshell.screens.find(s => s.name === WM.focusedMonitor?.name) ?? Quickshell.screens[0]
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "quickshell:altTab"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            color: "transparent"

            readonly property int cardWidth: 260
            readonly property int cardHeight: 210
            readonly property int padding: 14
            implicitWidth: Math.min(list.count * cardWidth, (screen?.width ?? 1920) * 0.85) + padding * 2
            implicitHeight: cardHeight + padding * 2

            StyledRectangularShadow {
                target: background
            }
            Rectangle {
                id: background
                anchors.fill: parent
                radius: Appearance.rounding.large
                color: Appearance.m3colors.m3surfaceContainer
                border.width: 1
                border.color: Appearance.colors.colLayer0Border

                ListView {
                    id: list
                    anchors.fill: parent
                    anchors.margins: panel.padding
                    orientation: ListView.Horizontal
                    clip: true
                    interactive: false
                    model: root.entries
                    currentIndex: root.index
                    highlightFollowsCurrentItem: false
                    onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)

                    delegate: Item {
                        id: cell
                        required property var modelData
                        required property int index
                        readonly property bool selected: cell.index === root.index
                        width: panel.cardWidth
                        height: panel.cardHeight

                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: 5
                            radius: Appearance.rounding.normal
                            color: cell.selected ? Appearance.colors.colSecondaryContainer
                                : mouse.containsMouse ? Appearance.colors.colLayer2Hover : "transparent"
                            border.width: cell.selected ? 2 : 0
                            border.color: Appearance.colors.colPrimary
                            Behavior on color {
                                animation: Appearance.animation.elementMoveFast.colorAnimation.createObject(this)
                            }

                            ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: 10
                                spacing: 8

                                Item {
                                    Layout.fillWidth: true
                                    Layout.fillHeight: true

                                    ScreencopyView {
                                        id: preview
                                        anchors.fill: parent
                                        live: true
                                        constraintSize: Qt.size(width, height)
                                        captureSource: WM.toplevelFor(cell.modelData)
                                        opacity: cell.modelData.minimized ? 0.5 : 1
                                    }
                                    // No preview (yet): the app's icon, big.
                                    IconImage {
                                        anchors.centerIn: parent
                                        visible: !preview.hasContent
                                        implicitSize: 64
                                        source: SystemAppearance.iconPath(AppSearch.guessIcon(cell.modelData.appId), "image-missing")
                                    }
                                }

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 8
                                    IconImage {
                                        implicitSize: 20
                                        source: SystemAppearance.iconPath(AppSearch.guessIcon(cell.modelData.appId), "image-missing")
                                    }
                                    StyledText {
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                        text: cell.modelData.title || cell.modelData.appId
                                        color: cell.selected ? Appearance.colors.colOnSecondaryContainer : Appearance.colors.colOnLayer1
                                    }
                                }
                            }

                            MouseArea {
                                id: mouse
                                anchors.fill: parent
                                hoverEnabled: true
                                onClicked: {
                                    root.index = cell.index;
                                    root.commit();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
