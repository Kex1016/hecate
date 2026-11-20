pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import qs
import qs.services
import qs.modules.common.widgets.haiiro as H

/**
 * hecate: the Alt+Tab switcher, the AltTab board of the haiiro canvas: a double-framed
 * `hecate: ~/windows` pane, one card per window (live preview, number, app and title) in most
 * recently used order, the selected card on overlay with a 5px double pine frame.
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
    // The panel stays mapped while the close frames play.
    property bool lingering: false

    function start(step) {
        const wins = WM.windowList.slice().sort((a, b) => b.focusSerial - a.focusSerial);
        if (wins.length === 0) return;
        root.entries = wins;
        root.index = step > 0 ? Math.min(1, wins.length - 1) : wins.length - 1;
        root.open = true;
        root.lingering = false;
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
        // The card pixelates out; the windows stay in `entries` until it is gone.
        root.lingering = root.shown;
        root.open = false;
        root.shown = false;
        showDelay.stop();
        if (!root.lingering) root.entries = [];
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
        active: root.shown || root.lingering
        sourceComponent: PanelWindow {
            id: panel
            screen: Quickshell.screens.find(s => s.name === WM.focusedMonitor?.name) ?? Quickshell.screens[0]
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "quickshell:altTab"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            color: "transparent"

            readonly property int cardWidth: 220
            readonly property int cardGap: 8
            readonly property int previewHeight: 130
            // padding 8 + preview + gap + one title line + padding 8
            readonly property int cardHeight: 8 + previewHeight + 8 + H.HTokens.lineBox(H.HTokens.fontLabel) + 8
            readonly property int maxListWidth: Math.floor((screen?.width ?? 1920) * 0.9) - window.inset * 2
            readonly property int listWidth: Math.min(root.entries.length * (cardWidth + cardGap) - cardGap, maxListWidth)

            anchors.top: true
            margins.top: Math.round((screen?.height ?? 1080) * 0.35)
            implicitWidth: window.implicitWidth + 2 * transition.margin
            implicitHeight: window.implicitHeight + 2 * transition.margin
            mask: Region { item: window }

            H.PixelTransition {
                id: transition
                x: transition.margin
                y: transition.margin
                width: window.implicitWidth
                height: window.implicitHeight
                shown: root.shown
                onClosed: {
                    root.lingering = false;
                    if (!root.open) root.entries = [];
                }

                H.HWindow {
                    id: window
                    width: implicitWidth
                    height: implicitHeight
                    label: "hecate: ~/windows"
                    hints: [
                        { key: "tab", text: "next" },
                        { key: "shift tab", text: "back" }
                    ]
                    footer: [
                        Row {
                            spacing: 8
                            H.HText {
                                anchors.verticalCenter: parent.verticalCenter
                                role: "caption"
                                ink: "inkMuted"
                                text: "release"
                            }
                            H.Keycap { key: "alt"; quiet: true; hint: "to switch" }
                        },
                        H.Keycap { key: "esc"; quiet: true; hint: "stay" }
                    ]

                    Item {
                        implicitWidth: panel.listWidth
                        implicitHeight: panel.cardHeight
                        Layout.fillWidth: true
                        Layout.preferredHeight: panel.cardHeight

                        ListView {
                            id: list
                            anchors.fill: parent
                            orientation: ListView.Horizontal
                            spacing: panel.cardGap
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
                                    color: cell.selected ? H.HTokens.overlay
                                        : mouse.containsMouse ? H.HTokens.highlightLow : "transparent"
                                }

                                Item {
                                    id: preview
                                    x: 8
                                    y: 8
                                    width: parent.width - 16
                                    height: panel.previewHeight

                                    // No preview (yet): the dither, like any picture that is missing.
                                    H.Dither {
                                        anchors.fill: parent
                                        visible: !capture.hasContent
                                        tone: cell.index % 2 === 0 ? "muted" : "lo"
                                        big: cell.selected
                                    }
                                    ScreencopyView {
                                        id: capture
                                        anchors.fill: parent
                                        anchors.margins: 2
                                        live: true
                                        constraintSize: Qt.size(width, height)
                                        captureSource: WM.toplevelFor(cell.modelData)
                                    }
                                    // 2px line, or the 5px double pine frame when selected
                                    Repeater {
                                        model: cell.selected ? [0, 3] : [0]
                                        Rectangle {
                                            required property int modelData
                                            x: modelData
                                            y: modelData
                                            width: preview.width - 2 * modelData
                                            height: preview.height - 2 * modelData
                                            color: "transparent"
                                            border.width: 2
                                            border.color: cell.selected ? H.HTokens.pine : H.HTokens.highlightHigh
                                        }
                                    }
                                    H.Chip {
                                        visible: cell.modelData.minimized
                                        x: 8
                                        y: 8
                                        text: "min"
                                        badge: true
                                    }
                                }

                                Row {
                                    x: 8
                                    y: preview.y + preview.height + 8
                                    width: parent.width - 16
                                    spacing: 0

                                    H.HText {
                                        id: marker
                                        role: "caption"
                                        ink: cell.selected ? "c4" : "c1"
                                        text: (cell.selected ? "»" : cell.index + 1) + " "
                                    }
                                    H.HText {
                                        id: appName
                                        visible: (cell.modelData.appId ?? "").length > 0
                                        role: "caption"
                                        ink: cell.selected ? "c5" : "subtle"
                                        text: cell.modelData.appId + ": "
                                    }
                                    H.HText {
                                        role: "caption"
                                        width: parent.width - marker.width - (appName.visible ? appName.width : 0)
                                        elide: Text.ElideRight
                                        text: cell.modelData.title || cell.modelData.appId
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
}
