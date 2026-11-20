import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

// system: who you are, how long the machine has been up, and what it runs.
DashTile {
    id: root

    title: "system"
    interactive: true

    Component.onCompleted: SystemInfo.refresh()

    RowLayout {
        spacing: 12
        H.Dither {
            implicitWidth: 48
            implicitHeight: 48
            tone: "pine"
        }
        ColumnLayout {
            spacing: 0
            H.HText {
                text: Config.options.profile.displayName || SystemInfo.username
                role: "valueLg"
                ink: "pine"
            }
            H.HText {
                text: "up " + DateTime.uptime.replace(/, /g, " ")
                role: "caption"
                ink: "muted"
            }
        }
    }

    GridLayout {
        Layout.fillWidth: true
        columns: 2
        columnSpacing: 12
        rowSpacing: 4

        component Key: H.HLabel { Layout.alignment: Qt.AlignVCenter }
        component Val: H.HText {
            Layout.fillWidth: true
            role: "code"
            elide: Text.ElideRight
        }

        Key { text: "os" }
        Val { text: SystemInfo.distroName + (WM.compositor ? " · " + WM.compositor : "") }
        Key { text: "kernel" }
        Val { text: SystemInfo.kernelVersion || "…" }
        Key { text: "shell" }
        Val { text: SystemInfo.shell || "…" }
        Key { text: "pkgs" }
        Val { text: SystemInfo.packages || "…" }
    }

    Item { Layout.fillHeight: true }
}
