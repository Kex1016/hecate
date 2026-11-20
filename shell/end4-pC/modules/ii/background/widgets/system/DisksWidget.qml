pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// disks · df -h for every mounted disk, one bar each; a disk over 90% turns love.
H.HWindow {
    id: root

    property bool live: true
    // at most this many disks; "/" always comes first
    property int maxRows: 6
    readonly property real fullAt: 0.9

    label: "df -h"
    widget: true
    implicitWidth: 360

    ConsumerTicket {
        service: ResourceUsage
        active: root.live
    }

    // "412 / 931G", "1.7 / 1.8T": both in the total's unit
    function usage(disk) {
        const units = ["K", "M", "G", "T", "P"];
        let total = disk.total, used = disk.used, i = 0;
        while (total >= 1024 && i < units.length - 1) {
            total /= 1024;
            used /= 1024;
            i++;
        }
        const fmt = v => v >= 10 ? `${Math.round(v)}` : v.toFixed(1);
        return `${fmt(used)} / ${fmt(total)}${units[i]}`;
    }

    Column {
        Layout.fillWidth: true
        spacing: 10

        Repeater {
            model: ResourceUsage.disks.slice(0, root.maxRows)

            H.Progress {
                required property var modelData
                width: parent.width
                label: modelData.mount
                count: root.usage(modelData)
                value: modelData.usedPercentage
                // nearly full: label, count, track and fill in love
                ink: modelData.usedPercentage > root.fullAt ? "love" : "text"
            }
        }
    }

    H.HText {
        visible: ResourceUsage.disks.length === 0
        ink: "c1"
        text: "no disks found"
    }
}
