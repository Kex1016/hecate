pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs
import qs.services
import qs.modules.common.widgets.haiiro

// "wlan 72" (signal in foam), "eth up", or "net off" / "net —" in muted.
// Hover: `net: <interface>` with the network, signal and addresses; click:
// the quick panel.
BarSegment {
    id: root

    readonly property bool wifiUp: Network.wifiEnabled && Network.wifiStatus === "connected"
    readonly property int strength: Network.active?.strength ?? Network.networkStrength ?? 0
    readonly property string kind: Network.ethernet ? "eth" : root.wifiUp || Network.wifiStatus === "connecting" ? "wlan" : "net"
    readonly property string value: Network.ethernet ? "up"
        : root.wifiUp ? String(root.strength)
        : Network.wifiStatus === "connecting" ? "…"
        : Network.wifiStatus === "disabled" ? "off" : "—"
    readonly property bool up: Network.ethernet || root.wifiUp

    popupLabel: "net: " + (Network.networkInterface || root.kind)
    popupWidth: 320
    onClicked: GlobalStates.sidebarRightOpen = !GlobalStates.sidebarRightOpen

    HText {
        text: root.kind
    }
    HText {
        text: root.value
        ink: root.up ? "foam" : "muted"
    }

    popup: Component {
        ColumnLayout {
            spacing: HTokens.space2

            Repeater {
                model: [
                    { label: "network", value: Network.ethernet ? "wired" : (Network.networkName || "none") },
                    { label: "signal", value: root.wifiUp ? root.strength + "%" : "" },
                    { label: "address", value: Network.ipAddress },
                    { label: "gateway", value: Network.gateway }
                ].filter(row => row.value.length > 0)

                Column {
                    id: kv
                    required property var modelData
                    Layout.fillWidth: true

                    HLabel {
                        text: kv.modelData.label
                    }
                    HText {
                        width: kv.width
                        text: kv.modelData.value
                        role: "valueLg"
                        elide: Text.ElideRight
                    }
                }
            }
        }
    }
}
