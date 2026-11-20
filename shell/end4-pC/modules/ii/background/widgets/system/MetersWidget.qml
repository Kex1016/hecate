pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// meters · cpu, ram and gpu as htop-style bars. The gpu bar is left out where the driver
// reports no busy figure (Intel without root).
H.HWindow {
    id: root

    property bool live: true

    label: "htop"
    widget: true
    implicitWidth: 360

    ConsumerTicket {
        service: ResourceUsage
        active: root.live
    }

    function percent(v) {
        return `${Math.round(Math.max(0, Math.min(1, v)) * 100)}%`;
    }

    Column {
        Layout.fillWidth: true
        spacing: 10

        H.Progress {
            width: parent.width
            label: "cpu"
            count: root.percent(ResourceUsage.cpuUsage)
            value: ResourceUsage.cpuUsage
        }
        H.Progress {
            width: parent.width
            label: "ram"
            count: root.percent(ResourceUsage.memoryUsedPercentage)
            value: ResourceUsage.memoryUsedPercentage
        }
        H.Progress {
            width: parent.width
            visible: ResourceUsage.gpuAvailable
            label: "gpu"
            count: root.percent(ResourceUsage.gpuUsage)
            value: ResourceUsage.gpuUsage
        }
    }
}
