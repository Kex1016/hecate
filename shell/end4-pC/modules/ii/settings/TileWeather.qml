pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

// weather: the temperature and sky now, the next three days when the
// weather service has a forecast, and sunset and wind in one quiet line.
DashTile {
    id: root

    title: "weather"
    sub: Weather.data.city && Weather.data.city !== "City" ? Weather.data.city.toLowerCase() : ""
    interactive: true

    readonly property bool on: Config.options.bar.weather.enable
    // [{ day: "fri", high: "11°", low: "4°" }], when the service provides it
    readonly property var days: (Weather.data.daily ?? []).slice(0, 3)

    // "4:05:00 PM" -> "16:05" (or "4:05 pm" with the 12h clock)
    function clock(s) {
        const m = String(s ?? "").match(/(\d+):(\d+)(?::\d+)?\s*([AP]M)?/i);
        if (!m) return "";
        let h = parseInt(m[1]);
        if (m[3]) h = (h % 12) + (m[3].toUpperCase() === "PM" ? 12 : 0);
        if (DateTime.use12HourFormat) return `${(h % 12) || 12}:${m[2]} ${h < 12 ? "am" : "pm"}`;
        return `${String(h).padStart(2, "0")}:${m[2]}`;
    }

    Row {
        visible: root.on
        spacing: H.HTokens.ch()
        H.HText {
            text: Weather.data.temp ?? ""
            role: "nav"
        }
        H.HText {
            anchors.bottom: parent.bottom
            text: Weather.data.description ?? ""
            ink: "foam"
        }
    }

    GridLayout {
        visible: root.on && root.days.length > 0
        Layout.fillWidth: true
        columns: 3
        columnSpacing: 8
        uniformCellWidths: true
        Repeater {
            model: root.days
            DashKv {
                required property var modelData
                Layout.fillWidth: true
                label: modelData.day
                value: modelData.high
                after: modelData.low
                large: false
            }
        }
    }

    GridLayout {
        visible: root.on && root.days.length === 0
        Layout.fillWidth: true
        columns: 3
        columnSpacing: 8
        uniformCellWidths: true
        DashKv { Layout.fillWidth: true; label: "feels"; value: Weather.data.tempFeelsLike ?? ""; large: false }
        DashKv { Layout.fillWidth: true; label: "humid"; value: Weather.data.humidity ?? ""; large: false }
        DashKv { Layout.fillWidth: true; label: "rain"; value: Weather.data.precip ?? ""; large: false }
    }

    H.HText {
        visible: root.on
        Layout.fillWidth: true
        text: [root.clock(Weather.data.sunset) ? "sunset " + root.clock(Weather.data.sunset) : "", Weather.data.wind ? "wind " + Weather.data.wind : ""].filter(s => s).join(" · ")
        role: "caption"
        ink: "muted"
        elide: Text.ElideRight
    }

    H.HText {
        visible: !root.on
        Layout.fillWidth: true
        text: "The sky is a mystery. Turn weather on under services."
        ink: "rose"
        wrapMode: Text.Wrap
    }
    Item { Layout.fillHeight: true }
}
