pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland

import qs.modules.common
import qs.modules.common.utils
import qs.modules.common.widgets
import qs.modules.common.widgets.haiiro as H
import qs.services

PanelWindow {
    id: root

    // Interface
    signal dismiss

    // Window props
    visible: false
    // color: Appearance.colors.colLayer0
    color: "black"
    WlrLayershell.namespace: "quickshell:regionSelector"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
    exclusionMode: ExclusionMode.Ignore
    anchors {
        left: true
        right: true
        top: true
        bottom: true
    }

    // Config
    readonly property string screenshotDir: Directories.screenshotTemp
    readonly property string screenshotPath: `${root.screenshotDir}/image-${screen.name}.png`

    // Preparation
    property bool screenshotReady: false

    function performTranslation() {
        screenshotReady = true;
    }

    TempScreenshotProcess {
        id: screenshotProc
        running: true
        screen: root.screen
        screenshotDir: root.screenshotDir
        screenshotPath: root.screenshotPath
        onExited: (_, __) => {
            root.visible = true;
            root.performTranslation();
        }
    }

    // Actual content
    property real scale: 1.0
    property real contentX: 0
    property real contentY: 0

    MouseArea {
        anchors.fill: parent
        clip: true

        property real lastX: 0
        property real lastY: 0

        cursorShape: Qt.SizeAllCursor

        onPressed: mouse => {
            lastX = mouse.x;
            lastY = mouse.y;
        }

        onPositionChanged: mouse => {
            if (pressed) {
                root.contentX += (mouse.x - lastX);
                root.contentY += (mouse.y - lastY);
                lastX = mouse.x;
                lastY = mouse.y;
            }
        }

        onWheel: event => {
            const zoomFactor = event.angleDelta.y > 0 ? 1.1 : 0.9;
            const oldScale = root.scale;
            const newScale = Math.min(Math.max(0.1, oldScale * zoomFactor), 5);

            if (newScale !== oldScale) {
                // Determine mouse position relative to the content's unscaled origin
                const localX = (event.x - root.contentX) / oldScale;
                const localY = (event.y - root.contentY) / oldScale;

                // Apply zoom
                root.scale = newScale;

                // Shift offsets to keep the same local point under the cursor
                root.contentX = event.x - (localX * newScale);
                root.contentY = event.y - (localY * newScale);
            }
        }

        ScreencopyView { // Freeze screen
            id: screencopy
            width: parent.width
            height: parent.height

            x: root.contentX
            y: root.contentY
            scale: root.scale
            transformOrigin: Item.TopLeft

            live: false
            captureSource: root.screen
        }

        Loader {
            width: parent.width * root.scale
            height: parent.height * root.scale

            x: root.contentX
            y: root.contentY

            active: root.screenshotReady
            sourceComponent: ScreenTextOverlay {
                screenshotPath: root.screenshotPath
                scaleFactor: root.scale
            }
        }
    }

    // The bar: `hecate: ~/translate`, a key button that opens the field for the service account
    // key, and the keys.
    H.PixelTransition {
        id: barTransition
        shown: root.visible
        anchors {
            horizontalCenter: parent.horizontalCenter
            bottom: parent.bottom
            bottomMargin: 40
        }
        width: bar.implicitWidth
        height: bar.implicitHeight

        H.HWindow {
            id: bar
            width: implicitWidth
            height: implicitHeight
            label: "hecate: ~/translate"
            variant: keyField.invalid ? "love" : "pine"
            hints: [
                { key: "scroll", text: "zoom" },
                { key: "drag", text: "move" },
                { key: "esc", text: "close" }
            ]

            focus: root.visible
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Escape) {
                    root.dismiss();
                    event.accepted = true;
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                H.InkButton {
                    text: "key"
                    ink: keyRow.shown ? "c5" : ""
                    flat: !keyRow.shown
                    on: keyRow.shown
                    activeFocusOnTab: false
                    onClicked: {
                        keyRow.shown = !keyRow.shown;
                        if (keyRow.shown) keyField.forceInputFocus();
                    }
                }
                H.HText {
                    visible: !keyRow.shown
                    role: "caption"
                    ink: "c1"
                    text: "Google Cloud service account key"
                }

                RowLayout {
                    id: keyRow
                    property bool shown: false
                    visible: shown
                    Layout.fillWidth: true
                    spacing: 8

                    H.PromptLine {
                        id: keyField
                        property bool invalid: false
                        Layout.fillWidth: true
                        Layout.preferredWidth: 400
                        path: "~/key"
                        input: true
                        // the key is a long JSON; paste it, then enter
                        field.inputMethodHints: Qt.ImhSensitiveData
                        onTextChanged: invalid = false
                        onAccepted: {
                            if (GoogleCloud.setKeyJson(text)) {
                                text = "";
                                keyRow.shown = false;
                            } else {
                                invalid = true;
                            }
                        }
                    }
                    H.HText {
                        visible: keyField.invalid
                        role: "caption"
                        ink: "c2"
                        text: "not a key"
                    }
                }
            }
        }
    }
}
