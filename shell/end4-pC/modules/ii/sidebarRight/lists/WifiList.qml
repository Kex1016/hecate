pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.services.network
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts
import Quickshell

// Networks in range, strongest first; the connected one on the overlay band.
// A secured network that needs its password asks for it on the next line.
ColumnLayout {
    id: root

    readonly property var actions: [
        { text: Network.wifiScanning ? "scanning" : "rescan", run: () => Network.rescanWifi() },
        { text: "settings", run: () => {
            GlobalStates.sidebarRightOpen = false;
            Quickshell.execDetached(["bash", "-c", Network.ethernet ? Config.options.apps.networkEthernet : Config.options.apps.network]);
        } }
    ]

    spacing: H.HTokens.space2

    Component.onCompleted: {
        Network.enableWifi();
        Network.rescanWifi();
    }

    H.HText {
        visible: Network.ethernet
        Layout.fillWidth: true
        text: "On a cable" + (Network.networkName ? ": " + Network.networkName : "") + "."
        ink: "c6"
        elide: Text.ElideRight
    }

    ListView {
        id: list
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: ScriptModel {
            values: Network.friendlyWifiNetworks
        }
        delegate: ColumnLayout {
            id: entry
            required property WifiAccessPoint modelData
            readonly property bool connecting: Network.wifiConnectTarget === modelData && !modelData.active
            width: list.width
            spacing: 0

            H.ListRow {
                Layout.fillWidth: true
                text: entry.modelData.ssid || "hidden network"
                selected: entry.modelData.active
                ink: entry.modelData.active ? "c6" : "text"
                meta: (entry.connecting ? "connecting · " : entry.modelData.active ? "connected · " : "")
                    + (entry.modelData.isSecure ? "secured · " : "open · ") + entry.modelData.strength + "%"
                enabled: !entry.connecting
                onClicked: {
                    if (entry.modelData.active)
                        Network.disconnectWifiNetwork();
                    else
                        Network.connectToWifiNetwork(entry.modelData);
                }
            }

            // password for this network
            H.PromptLine {
                id: password
                visible: entry.modelData.askingPassword
                Layout.fillWidth: true
                Layout.leftMargin: 8 + H.HTokens.ch() + 12
                Layout.topMargin: H.HTokens.space1
                Layout.bottomMargin: H.HTokens.space2
                path: "password"
                input: true
                field.echoMode: TextInput.Password
                field.inputMethodHints: Qt.ImhSensitiveData
                onVisibleChanged: if (visible) forceInputFocus()
                onAccepted: Network.changePassword(entry.modelData, password.text)
                Keys.onEscapePressed: event => {
                    entry.modelData.askingPassword = false;
                    event.accepted = true;
                }
            }
        }

        H.HText {
            visible: list.count === 0 && !Network.ethernet
            width: list.width
            text: Network.wifiScanning ? "Listening for networks..." : "No networks in range."
            ink: "c4"
            wrapMode: Text.Wrap
        }
    }
}
