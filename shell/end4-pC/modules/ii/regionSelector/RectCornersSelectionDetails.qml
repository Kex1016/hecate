import qs.modules.common
import qs.modules.common.widgets
import qs.modules.common.widgets.haiiro as H
import QtQuick

// The selection of the Region board: a 1px dashed text outline, the rest of the screen under
// the veil, an 8px square handle on each corner and a surface tag above it with the size and
// the position.
Item {
    id: root
    required property real regionX
    required property real regionY
    required property real regionWidth
    required property real regionHeight
    required property real mouseX
    required property real mouseY
    required property color color
    required property color overlayColor
    property bool showAimLines: Config.options.regionSelector.rect.showAimLines

    property bool breathingBorderOnly: false
    readonly property bool hasRegion: root.regionWidth >= 1 && root.regionHeight >= 1

    // Everything outside the region
    Rectangle {
        id: darkenOverlay
        z: 1
        visible: !root.breathingBorderOnly
        anchors {
            left: parent.left
            top: parent.top
            leftMargin: root.regionX - darkenOverlay.border.width
            topMargin: root.regionY - darkenOverlay.border.width
        }
        width: root.regionWidth + darkenOverlay.border.width * 2
        height: root.regionHeight + darkenOverlay.border.width * 2
        color: "transparent"
        border.color: root.overlayColor
        border.width: Math.max(root.width, root.height)
    }

    DashedBorder {
        id: selectionBorder
        z: 9
        visible: root.hasRegion
        anchors {
            left: parent.left
            top: parent.top
            leftMargin: Math.round(root.regionX) - borderWidth
            topMargin: Math.round(root.regionY) - borderWidth
        }
        width: Math.round(root.regionWidth) + borderWidth * 2
        height: Math.round(root.regionHeight) + borderWidth * 2

        color: root.color
        dashLength: 4
        gapLength: 4
        borderWidth: 1
    }

    // corner handles
    Repeater {
        model: root.hasRegion && !root.breathingBorderOnly ? [[0, 0], [1, 0], [0, 1], [1, 1]] : []
        Rectangle {
            required property var modelData
            z: 10
            width: 8
            height: 8
            x: Math.round(root.regionX + modelData[0] * root.regionWidth) - 4
            y: Math.round(root.regionY + modelData[1] * root.regionHeight) - 4
            color: H.HTokens.text
        }
    }

    // `520 × 300 at 240, 220`
    Rectangle {
        id: sizeTag
        z: 11
        visible: root.hasRegion && !root.breathingBorderOnly
        readonly property bool above: root.regionY >= height + 8
        x: Math.round(Math.max(0, Math.min(root.regionX, root.width - width)))
        y: Math.round(above ? root.regionY - height - 4 : root.regionY + root.regionHeight + 8)
        width: sizeText.implicitWidth + 16
        height: sizeText.implicitHeight + 4
        color: H.HTokens.surface

        Row {
            id: sizeText
            x: 8
            y: 2
            readonly property real implicitWidth: childrenRect.width
            readonly property real implicitHeight: childrenRect.height
            H.HText {
                role: "caption"
                text: `${Math.round(root.regionWidth)} × ${Math.round(root.regionHeight)} `
            }
            H.HText {
                role: "caption"
                ink: "c1"
                text: `at ${Math.round(root.regionX)}, ${Math.round(root.regionY)}`
            }
        }
    }

    // Aim lines
    Rectangle { // Vertical
        visible: root.showAimLines && !root.breathingBorderOnly
        z: 2
        x: Math.round(root.mouseX)
        anchors {
            top: parent.top
            bottom: parent.bottom
        }
        width: 1
        color: H.HTokens.inkHairline
    }
    Rectangle { // Horizontal
        visible: root.showAimLines && !root.breathingBorderOnly
        z: 2
        y: Math.round(root.mouseY)
        anchors {
            left: parent.left
            right: parent.right
        }
        height: 1
        color: H.HTokens.inkHairline
    }
}
