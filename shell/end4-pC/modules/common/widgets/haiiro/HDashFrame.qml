import QtQuick
import QtQuick.Shapes
import qs.modules.common.widgets.haiiro

// A 1px frame, solid or dashed (3px dashes, 3px gaps) for things that are
// inactive or coming soon. Crisp: no antialiasing, drawn on pixel centres.
Item {
    id: root

    property color color: HTokens.text
    property bool dashed: false

    Rectangle {
        visible: !root.dashed
        anchors.fill: parent
        color: "transparent"
        border.width: HTokens.borderHairline
        border.color: root.color
    }

    Shape {
        visible: root.dashed
        anchors.fill: parent
        antialiasing: false
        preferredRendererType: Shape.GeometryRenderer

        ShapePath {
            strokeColor: root.color
            strokeWidth: 1
            fillColor: "transparent"
            strokeStyle: ShapePath.DashLine
            dashPattern: [3, 3]
            capStyle: ShapePath.FlatCap
            joinStyle: ShapePath.MiterJoin
            startX: 0.5
            startY: 0.5
            PathLine { x: root.width - 0.5; y: 0.5 }
            PathLine { x: root.width - 0.5; y: root.height - 0.5 }
            PathLine { x: 0.5; y: root.height - 0.5 }
            PathLine { x: 0.5; y: 0.5 }
        }
    }
}
