pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Bluetooth

// Connected devices first, then paired, then the ones nearby. Opening the
// list turns the adapter on and scans until it closes. Click connects or
// disconnects (and pairs a new device); forget unpairs.
ColumnLayout {
    id: root

    readonly property var adapter: Bluetooth.defaultAdapter
    readonly property var devices: [...BluetoothStatus.connectedDevices, ...BluetoothStatus.pairedButNotConnectedDevices, ...BluetoothStatus.unpairedDevices]
    readonly property var actions: [
        { text: root.adapter?.discovering ? "scanning" : "scan", run: () => {
            if (root.adapter)
                root.adapter.discovering = !root.adapter.discovering;
        } },
        { text: "settings", run: () => {
            GlobalStates.sidebarRightOpen = false;
            Quickshell.execDetached(["bash", "-c", Config.options.apps.bluetooth]);
        } }
    ]

    spacing: H.HTokens.space2

    Component.onCompleted: {
        if (!root.adapter)
            return;
        root.adapter.enabled = true;
        root.adapter.discovering = true;
    }
    Component.onDestruction: {
        if (root.adapter)
            root.adapter.discovering = false;
    }

    ListView {
        id: list
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: ScriptModel {
            values: root.devices
        }
        delegate: Item {
            id: entry
            required property var modelData
            readonly property bool connected: BluetoothStatus.isConnected(modelData)
            width: list.width
            implicitHeight: row.implicitHeight

            H.ListRow {
                id: row
                anchors.fill: parent
                text: entry.modelData.name || entry.modelData.address || "unknown device"
                selected: entry.connected
                ink: entry.connected ? "c6" : "text"
                meta: {
                    const parts = [];
                    if (entry.connected) parts.push("connected");
                    else if (entry.modelData.paired) parts.push("paired");
                    if (entry.modelData.batteryAvailable) parts.push(Math.round(entry.modelData.battery * 100) + "%");
                    return parts.join(" · ");
                }
                onClicked: {
                    const d = entry.modelData;
                    if (entry.connected)
                        d.disconnect();
                    else if (d.paired)
                        d.connect();
                    else
                        d.pair();
                }
            }

            // over the row, so the row's own click doesn't take it
            H.InkButton {
                id: forget
                visible: entry.modelData.paired && (row.hovered || forget.hovered)
                anchors.right: parent.right
                anchors.rightMargin: 8
                anchors.verticalCenter: parent.verticalCenter
                ink: "c2"
                text: "forget"
                onClicked: entry.modelData.forget()
            }
        }

        H.HText {
            visible: list.count === 0
            width: list.width
            text: BluetoothStatus.available ? "Nothing nearby is listening yet." : "This machine has no bluetooth adapter."
            ink: "c4"
            wrapMode: Text.Wrap
        }
    }
}
