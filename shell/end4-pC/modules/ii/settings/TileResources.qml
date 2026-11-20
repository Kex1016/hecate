import QtQuick
import QtQuick.Layouts
import Quickshell.Io
import qs.services
import qs.modules.common.widgets.haiiro as H

// resources: cpu, ram, gpu (when the driver reports a busy percentage) and the
// root disk, as Progress bars.
DashTile {
    id: root

    title: "resources"
    spacing: 10

    // ResourceUsage only polls while something shows it.
    Component.onCompleted: ResourceUsage.consumers += 1
    Component.onDestruction: ResourceUsage.consumers -= 1

    function gib(kb) {
        return (kb / 1048576).toFixed(kb >= 10 * 1048576 ? 0 : 1);
    }

    // amdgpu and i915/xe expose a busy percentage; nvidia does not (the row hides).
    property int gpuBusy: -1
    FileView {
        id: gpuFile
        path: "/sys/class/drm/card0/device/gpu_busy_percent"
        printErrors: false
        onLoaded: {
            const v = parseInt(gpuFile.text().trim());
            root.gpuBusy = isNaN(v) ? -1 : v;
        }
        onLoadFailed: root.gpuBusy = -1
    }
    Timer {
        interval: 2000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: gpuFile.reload()
    }

    H.Progress {
        Layout.fillWidth: true
        label: "cpu"
        count: Math.round(ResourceUsage.cpuUsage * 100) + "%" + (ResourceUsage.cpuTemp > 0 ? " · " + Math.round(ResourceUsage.cpuTemp) + "°" : "")
        value: ResourceUsage.cpuUsage
    }
    H.Progress {
        Layout.fillWidth: true
        label: "ram"
        count: root.gib(ResourceUsage.memoryUsed) + " / " + Math.round(ResourceUsage.memoryTotal / 1048576) + "G"
        value: ResourceUsage.memoryUsedPercentage
    }
    H.Progress {
        Layout.fillWidth: true
        visible: root.gpuBusy >= 0
        label: "gpu"
        count: root.gpuBusy + "%"
        value: root.gpuBusy / 100
    }
    H.Progress {
        Layout.fillWidth: true
        label: "disk /"
        count: Math.round(ResourceUsage.diskUsed / 1048576) + " / " + Math.round(ResourceUsage.diskTotal / 1048576) + "G"
        value: ResourceUsage.diskUsedPercentage
    }
    Item { Layout.fillHeight: true }
}
