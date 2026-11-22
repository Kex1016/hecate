import QtQuick
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro as H
import "../desk/SkyMath.js" as SkyMath

// `whoami`: a nameplate. A pine checker for the portrait, the name in the
// display face, level and class on one line, exp as ten squares.
H.FreeWidget {
    id: root

    label: "whoami"

    SystemClock {
        id: clock
        enabled: root.live
        precision: SystemClock.Minutes
    }
    readonly property real exp: SkyMath.dayProgress(clock.date)

    art: H.PixelChecker {
        ink: "pine"
        implicitWidth: 96
        implicitHeight: 96
    }

    H.FreeText {
        role: "display"
        size: 64
        ink: "pine"
        text: SystemInfo.username
    }
    Row {
        spacing: 8
        H.FreeText { role: "code"; ink: "subtle"; text: "lv" }
        H.FreeText { role: "code"; text: DesktopWidgets.systemVersion }
        H.FreeText { role: "code"; ink: "subtle"; text: "·" }
        H.FreeText { role: "code"; text: DesktopWidgets.klass + (SystemInfo.hostname ? " of " + SystemInfo.hostname : ""); }
    }
    Row {
        spacing: 12
        H.PixelCells {
            anchors.verticalCenter: parent.verticalCenter
            count: 10
            filled: Math.round(root.exp * 10)
        }
        H.FreeText {
            anchors.verticalCenter: parent.verticalCenter
            role: "caption"
            ink: "subtle"
            text: "exp " + Math.round(root.exp * 100) + "%"
        }
    }
}
