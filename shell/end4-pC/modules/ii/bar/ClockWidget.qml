pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro

// The clock and the date in the middle of the bar ("21:34 thu 19 nov").
// 24h or 12h and the seconds are the only bar settings (time.format,
// time.secondPrecision). Hover: the weather, or the month when weather is
// off; click: the quick panel, which holds the calendar.
BarSegment {
    id: root

    readonly property bool seconds: Config.options.time.secondPrecision
    readonly property string format: {
        const f = Config.options.time.format || "hh:mm";
        return root.seconds && f.indexOf("ss") < 0 ? f.replace("mm", "mm:ss") : f;
    }
    readonly property bool weather: Config.options.bar.weather.enable

    spacing: HTokens.space4
    popupLabel: root.weather ? "weather" : "cal"
    popupWidth: root.weather ? 360 : 300
    onClicked: GlobalStates.sidebarRightOpen = !GlobalStates.sidebarRightOpen

    SystemClock {
        id: clock
        precision: root.seconds ? SystemClock.Seconds : SystemClock.Minutes
    }

    HText {
        text: Qt.locale("en_GB").toString(clock.date, root.format).toLowerCase()
    }
    HText {
        text: Qt.locale("en_GB").toString(clock.date, "ddd d MMM").toLowerCase()
        ink: "muted"
    }

    popup: root.weather ? weatherWindow : monthWindow

    Component {
        id: monthWindow
        CalendarGrid {
            year: clock.date.getFullYear()
            month: clock.date.getMonth()
            today: clock.date
        }
    }

    Component {
        id: weatherWindow
        ColumnLayout {
            id: sky
            spacing: HTokens.space2

            // a line of narration for the sky outside, by OpenWeather condition code
            readonly property int code: Number(Weather.data?.wCode ?? 0)
            readonly property string narration: {
                const c = sky.code;
                if (c >= 200 && c < 300) return "Thunder rolls somewhere past the rooftops.";
                if (c >= 300 && c < 400) return "A fine drizzle mists the glass.";
                if (c >= 500 && c < 600) return "You hear rain against the window.";
                if (c >= 600 && c < 700) return "Snow settles on the window sill.";
                if (c >= 700 && c < 800) return "The street lights blur in the haze.";
                if (c === 800) return "The sky is clear.";
                if (c > 800) return "Clouds drift over the city.";
                return "The weather has not reported in yet.";
            }

            Row {
                spacing: HTokens.ch()
                HText {
                    anchors.baseline: desc.baseline
                    text: Weather.data?.temp || "--"
                    role: "nav"
                }
                HText {
                    id: desc
                    text: String(Weather.data?.description ?? "").toLowerCase()
                    ink: "foam"
                }
            }
            HText {
                Layout.fillWidth: true
                text: sky.narration
                ink: "rose"
                wrapMode: Text.Wrap
            }

            // the stat panel: hairline cells
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: cells.implicitHeight + 2
                color: "transparent"
                border.width: HTokens.borderHairline
                border.color: HTokens.text

                RowLayout {
                    id: cells
                    x: 1
                    y: 1
                    width: parent.width - 2
                    spacing: 0

                    Repeater {
                        model: [
                            { label: "feels", value: Weather.data?.tempFeelsLike || "--" },
                            { label: "humid", value: Weather.data?.humidity || "--" },
                            { label: "wind", value: Weather.data?.wind || "--" }
                        ]
                        Rectangle {
                            id: cell
                            required property var modelData
                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            implicitHeight: kv.implicitHeight + 2 * HTokens.space2
                            color: "transparent"
                            border.width: HTokens.borderHairline
                            border.color: HTokens.text

                            Column {
                                id: kv
                                x: HTokens.space2
                                y: HTokens.space2
                                width: parent.width - 2 * HTokens.space2
                                HLabel {
                                    text: cell.modelData.label
                                }
                                HText {
                                    width: parent.width
                                    text: cell.modelData.value
                                    elide: Text.ElideRight
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
