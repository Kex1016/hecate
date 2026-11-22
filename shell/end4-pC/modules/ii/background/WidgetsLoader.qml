pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
// The widgets are loaded by file name; importing their directories is what makes
// quickshell index them, so the helpers they share there (ConsumerTicket) resolve.
import qs.modules.ii.background.widgets.desk as Desk
import qs.modules.ii.background.widgets.system as System
// the Claude and Apple widgets sit in widgets/ itself (ClaudeRunner is theirs)
import qs.modules.ii.background.widgets as Cards

// Places the desktop widgets. Which ones are on, and where, is
// DesktopWidgets' state. Each one loads in its form: free (straight on the
// background, the default) or window, from Settings › desktop widgets ›
// widget style, or just for that one from a right-click on its label. A free
// file that is missing or fails to load shows the window instead.
// Drag a free widget by its prompt line, a window by its top edge, the
// display clock by all of it: it snaps to an 8px grid and stays where it is
// dropped. Right-click the desktop to add or remove one.
Item {
    id: root
    anchors.fill: parent

    // Background.qml hands these over; the haiiro widgets need none of them
    property var screen: null
    property var wallpaperItem: null
    property bool wallpaperSafetyTriggered: false

    property int topZ: 1
    property bool menuOpen: false
    property point menuAt: Qt.point(0, 0)
    // the widget whose label was right-clicked, "" when its menu is closed
    property string formMenuKey: ""
    property point formMenuAt: Qt.point(0, 0)

    function closeMenu() {
        root.menuOpen = false;
        root.formMenuKey = "";
    }

    // right-click on empty desktop opens the menu; left clicks fall through
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.RightButton
        enabled: !GlobalStates.screenLocked
        onPressed: mouse => {
            root.menuAt = Qt.point(mouse.x, mouse.y);
            root.menuOpen = true;
        }
    }

    Repeater {
        model: DesktopWidgets.catalog

        delegate: Item {
            id: slot
            required property var modelData
            required property int index

            readonly property bool wanted: DesktopWidgets.loaded
                && DesktopWidgets.isEnabled(modelData.key)
                && (!GlobalStates.screenLocked || modelData.key === "clock")
            // the form it is in, and whether the free file is what loaded
            readonly property bool wantsFree: DesktopWidgets.formOf(modelData.key) === "free"
            property bool freeFailed: false
            readonly property string source: DesktopWidgets.sourceOf(modelData.key, wantsFree && !freeFailed)
            readonly property bool free: source !== "" && source === modelData.free
            readonly property var item: body.item
            // a free widget is dragged by its prompt line (handle); one without
            // a label, and the window-form clock, by all of it
            readonly property Item handle: free && item?.handle && (item?.label ?? "") !== "" ? item.handle : null
            readonly property bool bare: !handle && (modelData.key === "clock" || free)
            readonly property point home: DesktopWidgets.position(modelData.key, root.width, root.height, width, height)

            width: widget.implicitWidth
            height: widget.implicitHeight
            visible: widget.present
            z: DesktopWidgets.zOf(modelData.key)

            function rehome() {
                x = Qt.binding(() => slot.home.x);
                y = Qt.binding(() => slot.home.y);
            }

            H.PixelTransition {
                id: widget
                // wait for the free-file check, so a widget doesn't load twice
                shown: slot.wanted && DesktopWidgets.freeChecked
                index: slot.index % 4
                // a free widget's clearing reaches past its box; keep it while pixelating
                margin: slot.free ? 40 : 9

                Loader {
                    id: body
                    // PixelTransition plays a close for every widget at start (its
                    // shown starts true), which loaded each one once; only a
                    // widget that was on needs its close frames
                    active: slot.wanted || (widget.present && slot.wasOn)
                    source: active && slot.source !== "" ? Qt.resolvedUrl("widgets/" + slot.source) : ""
                    onStatusChanged: {
                        if (status !== Loader.Error) return;
                        if (slot.free) {
                            console.warn("[WidgetsLoader] free form of", slot.modelData.key, "failed to load, showing the window:", slot.modelData.free);
                            // later: the source is still being set
                            Qt.callLater(() => slot.freeFailed = true);
                        } else {
                            console.warn("[WidgetsLoader] could not load", slot.source);
                        }
                    }
                }
            }

            // Drag: by the free widget's prompt line, a window's top edge, or
            // all of a bare widget. Right-click: show this one in the other form.
            MouseArea {
                id: grab
                parent: slot.handle ?? slot
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
                height: slot.handle || slot.bare ? parent.height : 22
                enabled: slot.wanted && !GlobalStates.screenLocked
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                // a right-press opens the menu, it never drags
                drag.target: pressedButtons & Qt.RightButton ? null : slot
                drag.minimumX: 0
                drag.minimumY: 0
                drag.maximumX: Math.max(0, root.width - slot.width)
                drag.maximumY: Math.max(0, root.height - slot.height)
                onPressed: mouse => {
                    root.closeMenu();
                    if (mouse.button === Qt.RightButton) {
                        // only when there is another form to show
                        if (!slot.free && !DesktopWidgets.hasFree(slot.modelData.key)) {
                            mouse.accepted = false;
                            return;
                        }
                        const p = grab.mapToItem(root, mouse.x, mouse.y);
                        root.formMenuAt = Qt.point(p.x, p.y);
                        root.formMenuKey = slot.modelData.key;
                        return;
                    }
                    root.topZ += 1;
                    slot.z = root.topZ;
                }
                onReleased: mouse => {
                    if (mouse.button !== Qt.LeftButton) return;
                    DesktopWidgets.setPosition(slot.modelData.key, slot.x, slot.y, slot.z);
                    slot.rehome();
                }
            }
            property bool wasOn: false
            onWantedChanged: if (wanted) wasOn = true
            Component.onCompleted: {
                wasOn = wanted;
                rehome();
            }
            // freeFailed stays for the session: the shell's files do not change
            // under it, so trying again would only warn again
        }
    }

    // a click anywhere else closes the menu and gives the keyboard back
    MouseArea {
        anchors.fill: parent
        visible: root.menuOpen || root.formMenuKey !== ""
        acceptedButtons: Qt.AllButtons
        onPressed: root.closeMenu()
    }

    H.PixelTransition {
        id: menu
        shown: root.menuOpen
        x: Math.max(0, Math.min(root.menuAt.x, root.width - width))
        y: Math.max(0, Math.min(root.menuAt.y, root.height - height))
        z: 100000
        visible: present

        H.HWindow {
            label: "widgets"
            implicitWidth: 460
            note: "click to add or remove"
            Flow {
                Layout.fillWidth: true
                spacing: 8
                Repeater {
                    model: DesktopWidgets.available
                    delegate: H.Chip {
                        required property var modelData
                        text: modelData.name.toLowerCase()
                        on: DesktopWidgets.isEnabled(modelData.key)
                        interactive: true
                        onClicked: DesktopWidgets.toggle(modelData.key)
                    }
                }
            }
        }
    }

    // one widget's form: the right-click menu on its label
    H.PixelTransition {
        id: formMenu
        readonly property var entry: DesktopWidgets.catalog.find(c => c.key === root.formMenuKey) ?? null
        property var shownEntry: null
        onEntryChanged: if (entry) shownEntry = entry
        readonly property bool toWindow: shownEntry ? DesktopWidgets.formOf(shownEntry.key) === "free" && DesktopWidgets.hasFree(shownEntry.key) : false
        shown: root.formMenuKey !== ""
        x: Math.max(0, Math.min(root.formMenuAt.x, root.width - width))
        y: Math.max(0, Math.min(root.formMenuAt.y, root.height - height))
        z: 100000
        visible: present

        H.HWindow {
            label: (formMenu.shownEntry?.name ?? "").toLowerCase()
            note: "just this one; the rest follow settings"
            H.InkButton {
                text: formMenu.toWindow ? "show as window" : "show free"
                ink: "c1"
                onClicked: {
                    const key = formMenu.shownEntry.key;
                    root.closeMenu();
                    DesktopWidgets.setForm(key, formMenu.toWindow ? "window" : "free");
                }
            }
        }
    }
}
