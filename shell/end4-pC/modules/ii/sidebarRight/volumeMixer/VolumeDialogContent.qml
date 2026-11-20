pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Services.Pipewire

// Where sound goes (or comes from) and how loud each app is: the devices,
// the default one on the overlay band, then a slider per app stream.
ColumnLayout {
    id: root

    required property bool isSink
    readonly property var devices: isSink ? Audio.outputDevices : Audio.inputDevices
    readonly property var apps: isSink ? Audio.outputAppNodes : Audio.inputAppNodes
    readonly property var current: isSink ? Pipewire.defaultAudioSink : Pipewire.defaultAudioSource
    readonly property var actions: [
        { text: "mixer", run: () => {
            GlobalStates.sidebarRightOpen = false;
            Quickshell.execDetached(["bash", "-c", Config.options.apps.volumeMixer]);
        } }
    ]

    spacing: H.HTokens.space2

    PwObjectTracker {
        objects: [...root.devices, ...root.apps]
    }

    H.HLabel {
        text: root.isSink ? "devices" : "microphones"
    }

    Repeater {
        model: ScriptModel {
            values: root.devices
        }
        H.ListRow {
            required property var modelData
            Layout.fillWidth: true
            text: Audio.friendlyDeviceName(modelData).toLowerCase()
            selected: modelData.id === root.current?.id
            meta: modelData.audio?.muted ? "muted" : Math.round((modelData.audio?.volume ?? 0) * 100) + "%"
            onClicked: root.isSink ? Audio.setDefaultSink(modelData) : Audio.setDefaultSource(modelData)
        }
    }

    H.HRule {
        Layout.fillWidth: true
    }

    H.HLabel {
        text: "apps"
    }

    ListView {
        id: list
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.preferredHeight: contentHeight
        clip: true
        spacing: H.HTokens.space3
        boundsBehavior: Flickable.StopAtBounds
        model: ScriptModel {
            values: root.apps
        }
        delegate: ColumnLayout {
            id: app
            required property var modelData
            width: list.width
            spacing: H.HTokens.space1

            RowLayout {
                Layout.fillWidth: true
                H.HText {
                    Layout.fillWidth: true
                    text: {
                        const name = Audio.appNodeDisplayName(app.modelData);
                        const media = app.modelData.properties["media.name"];
                        return (media !== undefined ? `${name} · ${media}` : name).toLowerCase();
                    }
                    role: "code"
                    ink: app.modelData.audio?.muted ? "c1" : "text"
                    elide: Text.ElideRight
                }
                H.InkButton {
                    flat: true
                    on: app.modelData.audio?.muted ?? false
                    text: app.modelData.audio?.muted ? "unmute" : "mute"
                    onClicked: app.modelData.audio.muted = !app.modelData.audio.muted
                }
            }
            H.Slider {
                Layout.fillWidth: true
                enabled: !(app.modelData.audio?.muted ?? false)
                value: app.modelData.audio?.volume ?? 0
                onMoved: v => app.modelData.audio.volume = v
            }
        }

        H.HText {
            visible: list.count === 0
            width: list.width
            text: root.isSink ? "Nothing is playing." : "Nothing is listening."
            ink: "c4"
        }
    }
}
