pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H
import qs.modules.ii.background.widgets.system as System

// network, free · the rate down in foam, the last two minutes as one mirrored spark (down
// above the line, up below it in iris), the rate up under it.
H.FreeWidget {
    id: root

    readonly property string iface: ResourceUsage.netInterface
    // columns shown, the newest on the right
    readonly property int columns: 30
    // what fills a half when nothing is moving, so an idle link shows as a flat line and
    // not as a full-height bar of a few bytes
    readonly property real floorRate: 32000

    label: root.iface.length > 0 ? `net: ${root.iface}` : "net"

    System.ConsumerTicket {
        service: ResourceUsage
        active: root.live
    }

    function tail(values) {
        const v = values.slice(-root.columns);
        return Array(root.columns - v.length).fill(0).concat(v);
    }

    H.FreeText {
        ink: "c6"
        text: `↓ ${ResourceUsage.formatRate(ResourceUsage.netRxRate)}`
    }

    H.MirrorSpark {
        down: root.tail(ResourceUsage.netRxHistory)
        up: root.tail(ResourceUsage.netTxHistory)
        downMax: Math.max(root.floorRate, ...Array.from(ResourceUsage.netRxHistory))
        upMax: Math.max(root.floorRate, ...Array.from(ResourceUsage.netTxHistory))
    }

    H.FreeText {
        ink: "c7"
        text: `↑ ${ResourceUsage.formatRate(ResourceUsage.netTxRate)}`
    }
}
