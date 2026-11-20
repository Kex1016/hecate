pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

// The dashboard's home page: the host name as a display title, the
// narrator's line, the clock and the logo, then the stat panel: system,
// resources, network and weather on top; now playing, the month, todo and
// timer, and CakeOS updates under them.
FocusScope {
    id: root

    signal openSettings(string id)
    signal openPanel(string name)

    readonly property string who: Config.options.profile.displayName || SystemInfo.username
    readonly property date now: DateTime.clock.date

    function greeting(h) {
        if (h < 5) return "Good night";
        if (h < 12) return "Good morning";
        if (h < 18) return "Good afternoon";
        return "Good evening";
    }
    function countWord(n) {
        const words = ["no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];
        return n < words.length ? words[n] : String(n);
    }
    readonly property string narration: {
        const n = Notifications.list.length;
        const waits = n === 0 ? "nothing waits for you."
            : `${root.countWord(n)} notification${n === 1 ? "" : "s"} wait${n === 1 ? "s" : ""} for you.`;
        return `${root.greeting(root.now.getHours())}, ${root.who}. The machine hums; ${waits}`;
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 18

        // header
        RowLayout {
            Layout.fillWidth: true
            spacing: H.HTokens.space6

            ColumnLayout {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignBottom
                spacing: 8

                DashTitle {
                    text: SystemInfo.hostname || "hecate"
                }
                H.HText {
                    Layout.fillWidth: true
                    text: root.narration
                    ink: "rose"
                    elide: Text.ElideRight
                }
            }

            ColumnLayout {
                Layout.alignment: Qt.AlignBottom
                spacing: 0
                H.HText {
                    Layout.alignment: Qt.AlignRight
                    text: DateTime.time
                    role: "nav"
                }
                H.HText {
                    Layout.alignment: Qt.AlignRight
                    text: Qt.locale("en_GB").toString(root.now, "ddd d MMM yyyy").toLowerCase()
                    ink: "muted"
                }
            }

            DashLogo {
                Layout.alignment: Qt.AlignBottom
            }
        }

        // the stat panel: eight columns, two rows
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: H.HTokens.surface
            border.width: H.HTokens.borderHairline
            border.color: H.HTokens.text

            // eight equal columns, two equal rows; each tile spans whole columns
            Item {
                id: grid
                anchors.fill: parent
                anchors.margins: H.HTokens.borderHairline
                readonly property real column: grid.width / 8
                readonly property real row: Math.round(grid.height / 2)

                TileSystem {
                    x: 0; y: 0; width: grid.column * 2; height: grid.row
                    onClicked: root.openSettings("about")
                }
                TileResources {
                    x: grid.column * 2; y: 0; width: grid.column * 2; height: grid.row
                }
                TileNetwork {
                    x: grid.column * 4; y: 0; width: grid.column * 2; height: grid.row
                }
                TileWeather {
                    x: grid.column * 6; y: 0; width: grid.width - x; height: grid.row
                    onClicked: root.openSettings("services")
                }
                TileNowPlaying {
                    x: 0; y: grid.row; width: grid.column * 3; height: grid.height - y
                    onClicked: root.openPanel("media")
                }
                TileCalendar {
                    x: grid.column * 3; y: grid.row; width: grid.column * 2; height: grid.height - y
                    onClicked: root.openSettings("calendars")
                }
                TileTodo {
                    x: grid.column * 5; y: grid.row; width: grid.column; height: grid.height - y
                }
                TileUpdates {
                    x: grid.column * 6; y: grid.row; width: grid.width - x; height: grid.height - y
                }
            }
        }
    }
}
