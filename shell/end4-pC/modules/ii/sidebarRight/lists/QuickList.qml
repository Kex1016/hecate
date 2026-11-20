pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import qs.modules.ii.sidebarRight
import qs.modules.ii.sidebarRight.volumeMixer
import QtQuick
import QtQuick.Layouts
import Quickshell

// The list a long-pressed chip opens, in place of the sliders and
// notifications: « back, its name, its own actions, then the list.
ColumnLayout {
    id: root

    required property string view

    readonly property var titles: ({
        "wifi": "wifi",
        "bluetooth": "bluetooth",
        "vpn": "vpn",
        "nightLight": "night light",
        "sink": "output",
        "source": "input"
    })

    spacing: 14

    RowLayout {
        Layout.fillWidth: true
        spacing: H.HTokens.space3

        H.InkButton {
            flat: true
            text: "« back"
            onClicked: QuickState.view = ""
        }
        H.HLabel {
            Layout.fillWidth: true
            text: root.titles[root.view] ?? root.view
        }
        // each list names its own: [{ text, run }]
        Repeater {
            model: body.item?.actions ?? []
            H.InkButton {
                required property var modelData
                flat: true
                text: modelData.text
                onClicked: modelData.run()
            }
        }
    }

    H.HRule {
        Layout.fillWidth: true
        Layout.topMargin: -8
        Layout.bottomMargin: -8
    }

    Loader {
        id: body
        Layout.fillWidth: true
        Layout.fillHeight: true
        sourceComponent: {
            switch (root.view) {
            case "wifi": return wifi;
            case "bluetooth": return bluetooth;
            case "vpn": return vpn;
            case "nightLight": return nightLight;
            case "sink": return sink;
            case "source": return source;
            default: return null;
            }
        }
    }

    Component { id: wifi; WifiList {} }
    Component { id: bluetooth; BluetoothList {} }
    Component { id: vpn; VpnList {} }
    Component { id: nightLight; NightLightList {} }
    Component { id: sink; VolumeDialogContent { isSink: true } }
    Component { id: source; VolumeDialogContent { isSink: false } }
}
