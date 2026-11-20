pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// weather · the temperature now, what it is doing, and the chance of rain for each of the
// next seven hours as square bars.
H.HWindow {
    id: root

    property bool live: true
    readonly property bool hasData: (Weather.data?.temp ?? "") !== ""
    readonly property var hours: Weather.hourly ?? []

    label: "weather"
    widget: true
    implicitWidth: 360

    RowLayout {
        Layout.fillWidth: true
        spacing: H.HTokens.ch()
        visible: root.hasData

        H.HText {
            Layout.alignment: Qt.AlignBaseline
            role: "nav"
            text: `${Weather.tempValue}°`
        }
        H.HText {
            Layout.alignment: Qt.AlignBaseline
            Layout.fillWidth: true
            ink: "c6"
            elide: Text.ElideRight
            text: (Weather.data?.description ?? "").toLowerCase()
        }
    }

    H.HText {
        visible: !root.hasData
        ink: "c1"
        text: "no weather yet"
    }

    H.Spark {
        Layout.fillWidth: true
        Layout.preferredHeight: implicitHeight
        visible: root.hours.length > 0
        values: root.hours.map(h => h.rain)
        labels: root.hours.map(h => h.hour)
        // the bars are the chance itself: a full bar is 100%
        max: 100
    }

    H.HText {
        visible: root.hours.length > 0
        role: "caption"
        ink: "c1"
        text: "rain chance per hour"
    }
}
