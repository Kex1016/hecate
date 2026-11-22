pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H
import qs.modules.ii.background.widgets.system as System

// df -h · the free form of the disks widget: a row of 25 squares per mount, 4% each. A disk
// over 90% turns love, label and squares both.
H.FreeWidget {
    id: root

    // at most this many disks; "/" always comes first
    property int maxRows: 6
    readonly property real fullAt: 0.9
    readonly property int squares: 25

    label: "df -h"
    implicitWidth: 360

    System.ConsumerTicket {
        service: ResourceUsage
        active: root.live
    }

    GridLayout {
        Layout.fillWidth: true
        Layout.topMargin: 4
        columns: 2
        columnSpacing: 16
        rowSpacing: 10

        Repeater {
            model: ResourceUsage.disks.slice(0, root.maxRows)

            Item {
                id: disk
                required property var modelData
                readonly property bool full: modelData.usedPercentage > root.fullAt

                Layout.columnSpan: 2
                Layout.fillWidth: true
                implicitHeight: cells.implicitHeight

                H.FreeText {
                    id: mount
                    width: 64
                    anchors.verticalCenter: cells.verticalCenter
                    elide: Text.ElideRight
                    ink: disk.full ? "love" : "text"
                    text: disk.modelData.mount
                }
                H.PixelCells {
                    id: cells
                    x: 80
                    count: root.squares
                    filled: Math.round(disk.modelData.usedPercentage * root.squares)
                    ink: disk.full ? "love" : "text"
                    large: true
                }
            }
        }
    }

    H.FreeText {
        visible: ResourceUsage.disks.length === 0
        role: "code"
        ink: "muted"
        text: "no disks found"
    }
    H.FreeText {
        visible: ResourceUsage.disks.length > 0
        role: "caption"
        ink: "subtle"
        text: "one square is 4% · love over 90%"
    }
}
