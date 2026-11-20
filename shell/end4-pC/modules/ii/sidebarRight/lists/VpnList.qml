pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts
import Quickshell

// WireGuard and OpenVPN profiles from NetworkManager. Click connects or
// disconnects; delete asks once more before it removes a profile.
ColumnLayout {
    id: root

    property string confirming: ""
    readonly property var actions: [
        { text: "import", run: () => Vpn.pickAndImport() }
    ]

    spacing: H.HTokens.space2

    Component.onCompleted: Vpn.refresh()

    ListView {
        id: list
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: ScriptModel {
            values: Vpn.profiles
        }
        delegate: Item {
            id: entry
            required property var modelData
            readonly property bool active: Vpn.activeUuids.includes(modelData.uuid)
            readonly property bool confirming: root.confirming === modelData.uuid
            width: list.width
            implicitHeight: row.implicitHeight
            enabled: !Vpn.busy

            H.ListRow {
                id: row
                anchors.fill: parent
                text: entry.modelData.name
                selected: entry.active
                ink: entry.active ? "c6" : "text"
                meta: (entry.active ? "connected · " : "") + (entry.modelData.type === "wireguard" ? "wireguard" : "openvpn")
                onClicked: {
                    root.confirming = "";
                    Vpn.connectTo(entry.modelData.uuid);
                }
            }

            // over the row, so the row's own click doesn't take it
            H.InkButton {
                id: del
                visible: row.hovered || del.hovered || entry.confirming
                anchors.right: parent.right
                anchors.rightMargin: 8
                anchors.verticalCenter: parent.verticalCenter
                ink: "c2"
                text: entry.confirming ? "delete?" : "delete"
                onClicked: {
                    if (!entry.confirming) {
                        root.confirming = entry.modelData.uuid;
                        return;
                    }
                    root.confirming = "";
                    const uuid = entry.modelData.uuid;
                    Qt.callLater(() => Vpn.remove(uuid));
                }
            }
        }

        H.HText {
            visible: list.count === 0
            width: list.width
            text: "No VPN profiles yet. Import a WireGuard or OpenVPN file."
            ink: "c4"
            wrapMode: Text.Wrap
        }
    }

    H.HText {
        visible: Vpn.busy
        text: "working..."
        role: "caption"
        ink: "c1"
    }
}
