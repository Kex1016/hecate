pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs
import qs.services
import qs.modules.common.widgets.haiiro

// The pine ☼ at the start of the bar. Hover: `htop` (cpu, memory, swap);
// click: the launcher.
BarSegment {
    id: root

    popupLabel: "htop: ~"
    popupWidth: 320
    onClicked: GlobalStates.overviewOpen = !GlobalStates.overviewOpen

    HText {
        text: "☼"
        ink: "c5"
    }

    popup: Component {
        ColumnLayout {
            id: htop
            spacing: 10
            Component.onCompleted: ResourceUsage.consumers++
            Component.onDestruction: ResourceUsage.consumers--

            function gib(kb) {
                return String(Number((kb / 1024 / 1024).toFixed(1)));
            }

            Progress {
                Layout.fillWidth: true
                label: "cpu"
                count: Math.round(ResourceUsage.cpuUsage * 100) + "%" + (ResourceUsage.cpuTemp > 0 ? " · " + Math.round(ResourceUsage.cpuTemp) + "°C" : "")
                value: ResourceUsage.cpuUsage
            }
            Progress {
                Layout.fillWidth: true
                label: "ram"
                count: htop.gib(ResourceUsage.memoryUsed) + " / " + Math.round(ResourceUsage.memoryTotal / 1024 / 1024) + " GiB"
                value: ResourceUsage.memoryUsedPercentage
            }
            Progress {
                Layout.fillWidth: true
                label: "swap"
                visible: ResourceUsage.swapTotal > 1
                count: htop.gib(ResourceUsage.swapUsed) + " / " + Math.round(ResourceUsage.swapTotal / 1024 / 1024) + " GiB"
                value: ResourceUsage.swapUsedPercentage
            }
        }
    }
}
