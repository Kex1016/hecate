pragma ComponentBehavior: Bound
import qs.modules.common
import qs.modules.common.widgets
import qs.modules.common.widgets.haiiro as H
import qs.services
import QtQuick

// A window, layer or picture the selector offers as a one-click target: a square outline that
// turns solid and gets a faint fill under the pointer. No fades, no rounding.
Rectangle {
    id: root
    required property var clientDimensions

    property color colBackground: H.HTokens.surface
    property color colForeground: H.HTokens.text
    property bool showLabel: Config.options.regionSelector.targetRegions.showLabel
    property bool showIcon: false
    property bool targeted: false
    property color borderColor
    property color fillColor: "transparent"
    property string text: ""
    property real textPadding: 8
    z: 2
    color: fillColor
    border.color: borderColor
    border.width: targeted ? 2 : 1
    radius: 0

    visible: opacity > 0
    x: clientDimensions.at[0]
    y: clientDimensions.at[1]
    width: clientDimensions.size[0]
    height: clientDimensions.size[1]

    // The label is shown for the targeted one too: it says what a click takes.
    Loader {
        anchors {
            top: parent.top
            left: parent.left
            topMargin: root.textPadding
            leftMargin: root.textPadding
        }

        active: root.showLabel || root.targeted
        sourceComponent: Rectangle {
            color: root.colBackground
            implicitWidth: label.implicitWidth + 16
            implicitHeight: label.implicitHeight + 4

            H.HText {
                id: label
                x: 8
                y: 2
                role: "caption"
                text: root.text
                color: root.colForeground
            }
        }
    }
}
