import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro as H
import "SkyMath.js" as SkyMath

// `whoami`: the character sheet. Level is the system version, exp is how
// far through the day it is.
H.HWindow {
    id: root

    property bool live: true
    SystemClock {
        id: clock
        enabled: root.live
        precision: SystemClock.Minutes
    }
    readonly property real exp: SkyMath.dayProgress(clock.date)

    widget: true
    label: "whoami"
    implicitWidth: 360

    RowLayout {
        spacing: 12
        H.Dither { tone: "pine"; implicitWidth: 48; implicitHeight: 48 }
        H.HText { role: "nav"; ink: "c5"; text: SystemInfo.username; elide: Text.ElideRight; Layout.fillWidth: true }
    }
    GridLayout {
        columns: 2
        columnSpacing: 12
        rowSpacing: 3
        Repeater {
            model: [
                ["race", DesktopWidgets.race],
                ["class", DesktopWidgets.klass],
                ["realm", SystemInfo.hostname],
                ["level", DesktopWidgets.systemVersion]
            ]
            delegate: Item {
                id: row
                required property var modelData
                Layout.columnSpan: 2
                Layout.fillWidth: true
                visible: row.modelData[1].length > 0
                implicitHeight: key.implicitHeight
                H.HText { id: key; role: "code"; ink: "inkMuted"; text: row.modelData[0] }
                H.HText { x: 7 * H.HTokens.ch(14) + 12; role: "code"; text: row.modelData[1]; width: parent.width - x; elide: Text.ElideRight }
            }
        }
    }
    H.Progress {
        Layout.fillWidth: true
        label: "exp · today"
        count: Math.round(root.exp * 100) + "%"
        value: root.exp
    }
}
