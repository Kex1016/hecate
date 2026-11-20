import qs
import qs.services
import qs.modules.common
import QtQuick
import QtQuick.Layouts
import Quickshell

// Volume, microphone and brightness. The device names open their lists.
ColumnLayout {
    id: root

    readonly property var brightnessMonitor: Brightness.getMonitorForScreen(root.QsWindow.window?.screen ?? null)

    spacing: 10

    QuickSlider {
        Layout.fillWidth: true
        label: "vol"
        detail: Audio.sink ? Audio.friendlyDeviceName(Audio.sink).toLowerCase() : ""
        list: "sink"
        enabled: Audio.sink?.audio ? true : false
        value: Audio.sink?.audio?.volume ?? 0
        onMoved: v => {
            if (Audio.sink?.audio)
                Audio.sink.audio.volume = v;
        }
        onListRequested: QuickState.view = list
    }

    QuickSlider {
        Layout.fillWidth: true
        label: "mic"
        list: "source"
        enabled: Audio.source?.audio ? true : false
        value: Audio.source?.audio?.volume ?? 0
        onMoved: v => {
            if (Audio.source?.audio)
                Audio.source.audio.volume = v;
        }
        onListRequested: QuickState.view = list
    }

    QuickSlider {
        Layout.fillWidth: true
        label: "brightness"
        list: "nightLight"
        enabled: root.brightnessMonitor?.ready ?? false
        value: isFinite(root.brightnessMonitor?.brightness) ? root.brightnessMonitor.brightness : 0
        onMoved: v => root.brightnessMonitor?.setBrightness(v)
        onListRequested: QuickState.view = list
    }
}
