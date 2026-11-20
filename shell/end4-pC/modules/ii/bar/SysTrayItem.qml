pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Services.SystemTray
import Quickshell.Widgets
import qs.modules.common.widgets.haiiro

// One tray icon, 16px, dithered to text and muted. Hover: a window named
// after the item with its tooltip (or its id). Left click activates it,
// middle click is its secondary action, right click opens its menu.
BarSegment {
    id: root

    required property SystemTrayItem item
    readonly property string name: (root.item?.tooltipTitle || root.item?.title || root.item?.id || "tray").toLowerCase()
    readonly property string tip: BarState.oneLine(root.item?.tooltipDescription)
    property bool menuOpen: false

    implicitWidth: 16
    popupLabel: root.name
    popupWidth: 260
    popupBlocked: root.menuOpen
    cursorShape: Qt.PointingHandCursor

    onClicked: mouse => {
        if (mouse.button === Qt.LeftButton) {
            root.item.activate();
        } else if (mouse.button === Qt.MiddleButton) {
            root.item.secondaryActivate();
        } else if (root.item.hasMenu) {
            root.menuOpen = !root.menuOpen;
        }
    }
    onWheel: event => root.item.scroll(event.angleDelta.y, false)

    Item {
        width: 16
        height: 16

        IconImage {
            id: icon
            anchors.fill: parent
            source: root.item?.icon ?? ""
            implicitSize: 16
        }

        // the GL renderer dithers; the software one shows the plain icon
        ShaderEffectSource {
            id: iconTexture
            sourceItem: dither.visible ? icon : null
            hideSource: dither.visible
            textureSize: Qt.size(16, 16)
            smooth: false
            visible: false
        }
        ShaderEffect {
            id: dither
            anchors.fill: parent
            visible: GraphicsInfo.api !== GraphicsInfo.Software
            property var source: iconTexture
            property color light: HTokens.text
            property color dark: HTokens.muted
            property size cells: Qt.size(16, 16)
            fragmentShader: Qt.resolvedUrl("shaders/trayDither.frag.qsb")
        }
    }

    popup: Component {
        HText {
            text: root.tip.length > 0 ? root.tip : (root.item?.id ?? "")
            ink: root.tip.length > 0 ? "text" : "inkMuted"
            wrapMode: Text.Wrap
        }
    }

    // the menu, opened by a right click and closed by a choice or a click
    // anywhere else
    SysTrayMenu {
        id: menu
        target: root
        handle: root.item?.menu ?? null
        label: root.name
        wanted: root.menuOpen
        onDone: root.menuOpen = false
    }

    HyprlandFocusGrab {
        active: root.menuOpen && menu.popupWindow !== null
        windows: [menu.popupWindow, root.QsWindow.window]
        onCleared: root.menuOpen = false
    }
}
