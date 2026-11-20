import qs
import qs.services
import qs.modules.common
import qs.modules.common.functions
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts

// user@host, how long it has been up, and lock / settings / power.
RowLayout {
    id: root

    spacing: H.HTokens.space3

    readonly property string user: Config.options.profile.displayName !== "" ? Config.options.profile.displayName : SystemInfo.username
    // DateTime.uptime reads "1d, 3h, 12m"
    readonly property string uptime: DateTime.uptime.replace(/, /g, " ")

    function closeThen(fn) {
        GlobalStates.sidebarRightOpen = false;
        fn();
    }

    H.Dither {
        tone: "pine"
        Layout.preferredWidth: 32
        Layout.preferredHeight: 32
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 0

        Row {
            Layout.fillWidth: true
            clip: true
            H.HText {
                text: root.user
                ink: "c5"
            }
            H.HText {
                text: "@"
                ink: "c1"
            }
            H.HText {
                text: SystemInfo.hostname
            }
        }
        H.HText {
            text: "up " + root.uptime
            role: "caption"
            ink: "c1"
        }
    }

    H.InkButton {
        text: "lock"
        ink: "c1"
        onClicked: root.closeThen(() => Session.lock())
    }
    H.InkButton {
        text: "settings"
        ink: "c5"
        onClicked: root.closeThen(() => GlobalStates.settingsOpen = true)
    }
    H.InkButton {
        text: "power"
        ink: "c2"
        onClicked: root.closeThen(() => GlobalStates.sessionOpen = true)
    }
}
