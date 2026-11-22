pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H
import qs.modules.ii.background.widgets.system as System

// meters, free · htop's own bars, in text: cpu, ram and gpu. The gpu line is left out where
// the driver reports no busy figure (Intel without root).
H.FreeWidget {
    id: root

    label: "htop"
    // the board's lines are 20px apart: an 18px line box and 8px
    spacing: 8

    System.ConsumerTicket {
        service: ResourceUsage
        active: root.live
    }

    function percent(v) {
        return `${Math.round(Math.max(0, Math.min(1, v || 0)) * 100)}%`.padStart(3, " ");
    }

    component Meter: Row {
        id: meter

        property string name: ""
        property real value: 0

        spacing: H.HTokens.ch()

        H.FreeText { text: meter.name; ink: "subtle" }
        H.TextBar { value: meter.value }
        H.FreeText { text: root.percent(meter.value); ink: "c6"; horizontalAlignment: Text.AlignRight }
    }

    Meter { name: "cpu"; value: ResourceUsage.cpuUsage }
    Meter { name: "ram"; value: ResourceUsage.memoryUsedPercentage }
    Meter { visible: ResourceUsage.gpuAvailable; name: "gpu"; value: ResourceUsage.gpuUsage }
}
