import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Hyprland

Scope {
    id: root
    property string protectionMessage: ""
    property var focusedScreen: Quickshell.screens.find(s => s.name === WM.focusedMonitor?.name)

    property string currentIndicator: "volume"
    onCurrentIndicatorChanged: GlobalStates.osdIndicatorType = currentIndicator
    property var indicators: [
        {
            id: "volume",
            sourceUrl: "indicators/VolumeIndicator.qml"
        },
        {
            id: "brightness",
            sourceUrl: "indicators/BrightnessIndicator.qml"
        },
        {
            id: "gamma",
            sourceUrl: "indicators/GammaIndicator.qml"
        },
        {
            id: "mic",
            sourceUrl: "indicators/MicIndicator.qml"
        },
        {
            id: "keyboard",
            sourceUrl: "indicators/KeyboardIndicator.qml"
        },
    ]

    function triggerOsd() {
        GlobalStates.osdVolumeOpen = true;
        osdTimeout.restart();
    }

    Timer {
        id: osdTimeout
        interval: Config.options.osd.timeout
        repeat: false
        running: false
        onTriggered: {
            GlobalStates.osdVolumeOpen = false;
            root.protectionMessage = "";
        }
    }

    Connections {
        target: Brightness
        function onBrightnessChanged() {
            root.protectionMessage = "";
            root.currentIndicator = "brightness";
            root.triggerOsd();
        }
    }

    Connections {
        target: Hyprsunset
        function onGammaChangeAttempt() {
            root.protectionMessage = "";
            root.currentIndicator = "gamma";
            root.triggerOsd();
        }
    }

    Connections {
        // Listen to volume changes
        target: Audio.sink?.audio ?? null
        function onVolumeChanged() {
            if (!Audio.ready)
                return;
            root.currentIndicator = "volume";
            root.triggerOsd();
        }
        function onMutedChanged() {
            if (!Audio.ready)
                return;
            root.currentIndicator = "volume";
            root.triggerOsd();
        }
    }

    property string layoutPrevious: ""
    property string layoutCurrent: HyprlandXkb.currentLayoutCode
    onLayoutCurrentChanged: {
        if (layoutCurrent === "") return;
        const was = layoutShown;
        layoutShown = layoutCurrent;
        if (was === "") return; // first value after start-up is not a change
        layoutPrevious = was;
        protectionMessage = "";
        currentIndicator = "keyboard";
        triggerOsd();
    }
    property string layoutShown: ""

    Connections {
        target: Audio.source?.audio ?? null
        function onVolumeChanged() {
            if (!Audio.ready) return;
            root.protectionMessage = "";
            root.currentIndicator = "mic";
            root.triggerOsd();
        }
        function onMutedChanged() {
            if (!Audio.ready) return;
            root.protectionMessage = "";
            root.currentIndicator = "mic";
            root.triggerOsd();
        }
    }

    Connections {
        // Listen to protection triggers
        target: Audio
        function onSinkProtectionTriggered(reason) {
            root.protectionMessage = reason;
            root.currentIndicator = "volume";
            root.triggerOsd();
        }
    }

    // Keep the window mapped until the close frames have played.
    property bool windowMapped: false
    Connections {
        target: GlobalStates
        function onOsdVolumeOpenChanged() {
            if (GlobalStates.osdVolumeOpen) root.windowMapped = true;
        }
    }

    Loader {
        id: osdLoader
        active: root.windowMapped

        sourceComponent: PanelWindow {
            id: osdRoot
            color: "transparent"
            readonly property int pad: 12 // room for the pixelate margin

            Connections {
                target: root
                function onFocusedScreenChanged() {
                    osdRoot.screen = root.focusedScreen;
                }
            }

            WlrLayershell.namespace: "quickshell:onScreenDisplay"
            WlrLayershell.layer: WlrLayer.Overlay
            anchors {
                bottom: true
            }
            mask: Region {
                item: osdFrame
            }

            exclusionMode: ExclusionMode.Ignore
            exclusiveZone: 0
            margins {
                bottom: 80 - pad
            }

            implicitWidth: osdFrame.width + 2 * pad
            implicitHeight: osdFrame.height + 2 * pad
            visible: osdLoader.active

            // One window at a time, bottom centre: it pixelates in, sits for
            // the timeout and pixelates out. Nothing slides.
            H.PixelTransition {
                id: osdFrame
                x: osdRoot.pad
                y: osdRoot.pad
                shown: GlobalStates.osdVolumeOpen
                width: H.HTokens.osdWidth
                height: content.implicitHeight
                onClosed: {
                    if (!GlobalStates.osdVolumeOpen) root.windowMapped = false;
                }

                Item {
                    id: content
                    width: osdFrame.width
                    implicitHeight: root.protectionMessage !== "" ? protection.implicitHeight : osdIndicatorLoader.implicitHeight

                    MouseArea {
                        anchors.fill: parent
                        hoverEnabled: true
                        onEntered: GlobalStates.osdVolumeOpen = false
                    }

                    Loader {
                        id: osdIndicatorLoader
                        visible: root.protectionMessage === ""
                        width: parent.width
                        source: root.indicators.find(i => i.id === root.currentIndicator)?.sourceUrl
                    }

                    Binding {
                        target: osdIndicatorLoader.item
                        property: "previous"
                        value: root.layoutPrevious
                        when: root.currentIndicator === "keyboard" && osdIndicatorLoader.item !== null
                    }
                    Binding {
                        target: osdIndicatorLoader.item
                        property: "current"
                        value: root.layoutCurrent
                        when: root.currentIndicator === "keyboard" && osdIndicatorLoader.item !== null
                    }

                    OsdValueIndicator {
                        id: protection
                        visible: root.protectionMessage !== ""
                        name: "vol"
                        variant: "love"
                        message: root.protectionMessage
                    }
                }
            }
        }
    }

    IpcHandler {
        target: "osdVolume"

        function trigger() {
            root.triggerOsd();
        }

        function hide() {
            GlobalStates.osdVolumeOpen = false;
        }

        function toggle() {
            GlobalStates.osdVolumeOpen = !GlobalStates.osdVolumeOpen;
        }
    }
    CompositorGlobalShortcut {
        name: "osdVolumeTrigger"
        description: "Triggers volume OSD on press"

        onPressed: {
            root.triggerOsd();
        }
    }
    CompositorGlobalShortcut {
        name: "osdVolumeHide"
        description: "Hides volume OSD on press"

        onPressed: {
            GlobalStates.osdVolumeOpen = false;
        }
    }
}
