pragma ComponentBehavior: Bound
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland

// What a region tool says after it ran: `ocr: copied`, `tr: ja » en`, `pick: copied`. A window in
// the top right corner that pixelates in, stays a few seconds and pixelates out; a click closes it.
PanelWindow {
    id: root

    property bool shown: false
    property string label: ""
    // pine, love (failed), gold (needs you), foam (done)
    property string variant: "foam"
    // [{ text: "...", ink: "c1" }], one per paragraph
    property var lines: []
    property color swatch: "transparent"
    // pushed down by the recording window when both are up
    property int topMargin: 56
    readonly property int bodyWidth: 380 - 2 * 20

    function show(label, variant, lines, swatch) {
        root.label = label;
        root.variant = variant;
        root.lines = lines;
        root.swatch = swatch ?? "transparent";
        root.shown = true;
        hide.restart();
    }

    visible: transition.present
    color: "transparent"
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "quickshell:regionToast"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    anchors {
        top: true
        right: true
    }
    margins.top: root.topMargin - transition.margin
    margins.right: 40 - transition.margin
    implicitWidth: window.width + 2 * transition.margin
    implicitHeight: window.implicitHeight + 2 * transition.margin
    mask: Region { item: window }

    Timer {
        id: hide
        interval: 6000
        onTriggered: root.shown = false
    }

    H.PixelTransition {
        id: transition
        x: transition.margin
        y: transition.margin
        width: window.width
        height: window.implicitHeight
        shown: root.shown

        H.HWindow {
            id: window
            width: 380
            height: implicitHeight
            label: root.label
            variant: root.variant
            widget: true

            Rectangle {
                visible: root.swatch.a > 0
                Layout.preferredWidth: root.bodyWidth
                Layout.preferredHeight: 48
                color: root.swatch
                border.width: 2
                border.color: H.HTokens.text
            }
            Repeater {
                model: root.lines
                H.HText {
                    required property var modelData
                    Layout.fillWidth: true
                    Layout.preferredWidth: root.bodyWidth
                    wrapMode: Text.Wrap
                    maximumLineCount: 8
                    elide: Text.ElideRight
                    ink: modelData.ink ?? "text"
                    role: modelData.role ?? "body"
                    text: modelData.text
                }
            }
        }

        MouseArea {
            x: 0
            y: 0
            width: window.width
            height: window.height
            onClicked: root.shown = false
        }
    }
}
