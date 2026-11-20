pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro

// One segment of the bar: a run of text in the VGA face. Resting the
// pointer on it opens its window (`popup`) under it; the window stays while
// the pointer is on the segment or the window. Once one window is open,
// moving along the bar swaps it straight away. Clicking closes the window
// and emits `clicked`, where the segment opens the panel it belongs to.
MouseArea {
    id: root

    // what the window holds; null for a segment with no window
    property Component popup: null
    property string popupLabel: ""
    property string popupVariant: "pine"
    property int popupWidth: 300
    property var popupHints: []
    // a menu or something else covers the window's place
    property bool popupBlocked: false
    property alias spacing: row.spacing
    default property alias content: row.data

    readonly property bool engaged: root.containsMouse || window.hovered
    readonly property bool popupOpen: BarState.openSegment === root

    hoverEnabled: true
    acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
    implicitWidth: row.implicitWidth
    implicitHeight: HTokens.barHeight - HTokens.barSeam
    Layout.fillHeight: true

    function closePopup() {
        openDelay.stop();
        if (BarState.openSegment === root)
            BarState.openSegment = null;
    }

    onEngagedChanged: {
        if (root.engaged && root.popup !== null && !root.popupBlocked) {
            closeDelay.stop();
            if (BarState.openSegment !== null && BarState.openSegment !== root)
                BarState.openSegment = root;
            else
                openDelay.restart();
        } else if (!root.engaged) {
            openDelay.stop();
            closeDelay.restart();
        }
    }
    onPopupBlockedChanged: if (root.popupBlocked) root.closePopup()
    onPressed: root.closePopup()
    Component.onDestruction: root.closePopup()

    Timer {
        id: openDelay
        interval: 250
        onTriggered: BarState.openSegment = root
    }

    Timer {
        id: closeDelay
        interval: 200
        onTriggered: if (!root.engaged) root.closePopup()
    }

    Row {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        height: childrenRect.height
        spacing: HTokens.ch()
    }

    BarPopup {
        id: window
        target: root
        wanted: root.popupOpen && !root.popupBlocked
        content: root.popup
        label: root.popupLabel
        variant: root.popupVariant
        windowWidth: root.popupWidth
        hints: root.popupHints
    }
}
