pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
// The widgets are loaded by file name; importing their directories is what makes
// quickshell index them, so the helpers they share there (ConsumerTicket, DiskBar) resolve.
import qs.modules.ii.background.widgets.desk as Desk
import qs.modules.ii.background.widgets.system as System

// Places the desktop widgets. Which ones are on, and where, is
// DesktopWidgets' state. Drag a widget by its top edge: it snaps to an 8px
// grid and stays where it is dropped. Right-click the desktop to add or
// remove one; that is the whole menu.
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

    function closeMenu() { root.menuOpen = false; }

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
            readonly property bool bare: modelData.key === "clock"
            readonly property point home: DesktopWidgets.position(modelData.key, root.width, root.height, width, height)

            width: widget.implicitWidth
            height: widget.implicitHeight
            visible: widget.present
            z: DesktopWidgets.zOf(modelData.key)

            function rehome() {
                x = Qt.binding(() => slot.home.x);
                y = Qt.binding(() => slot.home.y);
            }
            Component.onCompleted: rehome()

            H.PixelTransition {
                id: widget
                shown: slot.wanted
                index: slot.index % 4

                Loader {
                    active: slot.wanted || widget.present
                    source: active ? Qt.resolvedUrl("widgets/" + slot.modelData.file) : ""
                    onStatusChanged: if (status === Loader.Error) console.warn("[WidgetsLoader] could not load", slot.modelData.file)
                }
            }

            // drag by the top edge; the clock has no frame, so all of it
            MouseArea {
                id: grab
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
                height: slot.bare ? parent.height : 22
                enabled: slot.wanted && !GlobalStates.screenLocked
                cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                drag.target: slot
                drag.minimumX: 0
                drag.minimumY: 0
                drag.maximumX: Math.max(0, root.width - slot.width)
                drag.maximumY: Math.max(0, root.height - slot.height)
                onPressed: {
                    root.closeMenu();
                    root.topZ += 1;
                    slot.z = root.topZ;
                }
                onReleased: {
                    DesktopWidgets.setPosition(slot.modelData.key, slot.x, slot.y, slot.z);
                    slot.rehome();
                }
            }
        }
    }

    // a click anywhere else closes the menu and gives the keyboard back
    MouseArea {
        anchors.fill: parent
        visible: root.menuOpen
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
                    model: DesktopWidgets.catalog
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
}
