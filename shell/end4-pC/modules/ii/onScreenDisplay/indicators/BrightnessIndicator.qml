import qs.services
import QtQuick
import Quickshell
import qs.modules.ii.onScreenDisplay

OsdValueIndicator {
    id: root
    property var focusedScreen: Quickshell.screens.find(s => s.name === WM.focusedMonitor?.name)
    property var brightnessMonitor: Brightness.getMonitorForScreen(focusedScreen)

    name: "brightness"
    value: root.brightnessMonitor?.brightness ?? 0.5
}
