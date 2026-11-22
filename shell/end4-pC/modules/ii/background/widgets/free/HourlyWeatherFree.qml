pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// weather, free · the temperature at 120px in the display face, what it is doing beside
// it, and the chance of rain for each of the next hours as drops hanging from a cloud line.
H.FreeWidget {
    id: root

    readonly property bool hasData: (Weather.data?.temp ?? "") !== ""
    readonly property var hours: Weather.hourly ?? []

    label: "weather"

    Item {
        visible: root.hasData
        implicitWidth: temp.implicitWidth + 16 + description.implicitWidth
        implicitHeight: temp.implicitHeight

        H.FreeText {
            id: temp
            text: `${Weather.tempValue}°`
            role: "display"
            size: 120
        }
        H.FreeText {
            id: description
            x: temp.implicitWidth + 16
            anchors.bottom: parent.bottom
            // the board lifts it one line off the figure's bottom edge
            anchors.bottomMargin: 18
            ink: "c6"
            text: (Weather.data?.description ?? "").toLowerCase()
        }
    }

    H.FreeText {
        visible: !root.hasData
        ink: "c1"
        text: "no weather yet"
    }

    H.RainColumns {
        visible: root.hours.length > 0
        hours: root.hours.map(h => ({ hour: h.hour, chance: h.rain }))
        // the cloud line runs one gap past the last column, as on the board
        Layout.preferredWidth: implicitWidth + px(10)
    }
}
