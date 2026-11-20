import qs
import qs.services
import qs.modules.common
import QtQuick
import QtQuick.Layouts

// The nine toggles, three to a row. wifi, bluetooth, vpn and night light open
// their list in place on a long press. A toggle the machine can't do (no
// bluetooth adapter, no EasyEffects) stays in its cell, at 55%.
GridLayout {
    id: root

    columns: 3
    columnSpacing: 6
    rowSpacing: 6
    uniformCellWidths: true

    Component.onCompleted: {
        Hyprsunset.fetchState();
        EasyEffects.fetchActiveState();
    }

    QuickChip {
        Layout.fillWidth: true
        text: "wifi"
        on: Network.wifiStatus !== "disabled"
        list: "wifi"
        onToggled: Network.toggleWifi()
        onListRequested: QuickState.view = list
    }
    QuickChip {
        Layout.fillWidth: true
        text: "bluetooth"
        enabled: BluetoothStatus.available
        on: BluetoothStatus.enabled
        list: "bluetooth"
        onToggled: BluetoothStatus.togglePower()
        onListRequested: QuickState.view = list
    }
    QuickChip {
        Layout.fillWidth: true
        text: "vpn"
        on: Vpn.connected
        list: "vpn"
        // with no profile yet, Vpn asks for its list (dialogRequested)
        onToggled: Vpn.toggle()
        onListRequested: QuickState.view = list
    }
    QuickChip {
        Layout.fillWidth: true
        text: "night light"
        on: Hyprsunset.temperatureActive
        list: "nightLight"
        onToggled: Hyprsunset.toggleTemperature()
        onListRequested: QuickState.view = list
    }
    QuickChip {
        Layout.fillWidth: true
        text: "do not disturb"
        on: Notifications.silent
        onToggled: Notifications.silent = !Notifications.silent
    }
    QuickChip {
        Layout.fillWidth: true
        text: "idle inhibit"
        on: Idle.inhibit
        onToggled: Idle.toggleInhibit()
    }
    QuickChip {
        Layout.fillWidth: true
        text: "game mode"
        on: QuickState.gameMode
        onToggled: QuickState.toggleGameMode()
    }
    QuickChip {
        Layout.fillWidth: true
        text: "easyeffects"
        enabled: EasyEffects.available
        on: EasyEffects.active
        onToggled: EasyEffects.toggle()
    }
    QuickChip {
        Layout.fillWidth: true
        text: "mic off"
        on: Audio.source?.audio?.muted ?? false
        list: "source"
        onToggled: Audio.toggleMicMute()
        onListRequested: QuickState.view = list
    }
}
