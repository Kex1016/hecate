pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// network · download and upload on the default route's interface, and the last two
// minutes of download as a sparkline.
H.HWindow {
    id: root

    property bool live: true
    readonly property string iface: ResourceUsage.netInterface

    label: root.iface.length > 0 ? `net: ${root.iface}` : "net"
    widget: true
    implicitWidth: 360

    ConsumerTicket {
        service: ResourceUsage
        active: root.live
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 2 * H.HTokens.ch()

        H.HText {
            ink: "c6"
            text: `↓ ${ResourceUsage.formatRate(ResourceUsage.netRxRate)}`
        }
        H.HText {
            Layout.fillWidth: true
            ink: "c7"
            text: `↑ ${ResourceUsage.formatRate(ResourceUsage.netTxRate)}`
        }
    }

    Item {
        id: chart
        Layout.fillWidth: true
        Layout.preferredHeight: spark.implicitHeight

        H.Spark {
            id: spark
            anchors.right: parent.right
            height: parent.height
            // as many 6px columns as fit, the newest on the right
            values: ResourceUsage.netRxHistory.slice(-Math.max(1, Math.floor((chart.width + 2) / 8)))
            width: implicitWidth
        }
    }
}
