import qs.modules.common
import qs.modules.common.widgets
import QtQuick
import QtQuick.Shapes
import Quickshell

// The "free" target: a hand-drawn loop around what to take. Square caps, straight joins.
Item {
    id: root
    required property color color
    required property color overlayColor
    required property list<point> points
    property int strokeWidth: Config.options.regionSelector.circle.strokeWidth

    Rectangle {
        id: darkenOverlay
        z: 1
        anchors.fill: parent
        color: root.overlayColor
    }

    Shape {
        id: shape
        z: 2
        anchors.fill: parent
        layer.enabled: true
        layer.smooth: false
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            id: shapePath
            strokeWidth: root.strokeWidth
            pathHints: ShapePath.PathLinear
            fillColor: "transparent"
            strokeColor: root.color
            capStyle: ShapePath.SquareCap
            joinStyle: ShapePath.MiterJoin

            PathPolyline {
                path: root.points
            }
        }
    }
}
