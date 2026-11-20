import QtQuick
import QtQuick.Layouts
import Quickshell.Io
import qs.services
import qs.modules.common.widgets.haiiro as H

// network: the interface and signal, down and up rates from /proc/net/dev,
// a sparkline of the download rate, and VPN and Bluetooth in one quiet line.
DashTile {
    id: root

    title: "network"
    sub: {
        const iface = Network.networkInterface;
        if (!iface) return Network.ethernet ? "ethernet" : "offline";
        return Network.wifi && !Network.ethernet ? `${iface} · ${Network.networkStrength}%` : iface;
    }

    property real down: 0
    property real up: 0
    property var history: []
    property var last: null
    readonly property int samples: 22

    function rate(bytes) {
        if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + " MB/s";
        if (bytes >= 1024) return Math.round(bytes / 1024) + " KB/s";
        return Math.round(bytes) + " B/s";
    }

    // Totals over every interface but loopback and virtual ones.
    function totals(text) {
        let rx = 0, tx = 0;
        for (const line of text.split("\n").slice(2)) {
            const m = line.trim().match(/^([^:]+):\s*(.*)$/);
            if (!m || /^(lo|veth|docker|br-|virbr)/.test(m[1])) continue;
            const f = m[2].split(/\s+/).map(Number);
            rx += f[0] || 0;
            tx += f[8] || 0;
        }
        return { rx, tx, t: Date.now() };
    }

    FileView {
        id: dev
        path: "/proc/net/dev"
        printErrors: false
        onLoaded: {
            const now = root.totals(dev.text());
            if (root.last) {
                const dt = Math.max(0.001, (now.t - root.last.t) / 1000);
                root.down = Math.max(0, (now.rx - root.last.rx) / dt);
                root.up = Math.max(0, (now.tx - root.last.tx) / dt);
                root.history = root.history.concat([root.down]).slice(-root.samples);
            }
            root.last = now;
        }
    }
    Timer {
        interval: 1000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: dev.reload()
    }

    RowLayout {
        spacing: H.HTokens.space6
        DashKv { label: "down"; value: root.rate(root.down); ink: "foam" }
        DashKv { label: "up"; value: root.rate(root.up); ink: "iris" }
    }
    H.Spark {
        Layout.fillWidth: true
        values: root.history.length > 0 ? root.history : [0]
    }
    H.HText {
        Layout.fillWidth: true
        text: `vpn ${Vpn.connected ? Vpn.activeProfile?.name ?? "on" : "off"} · bluetooth: ${BluetoothStatus.enabled ? (BluetoothStatus.activeDeviceCount === 1 ? "1 device" : BluetoothStatus.activeDeviceCount + " devices") : "off"}`
        role: "caption"
        ink: "muted"
        elide: Text.ElideRight
    }
    Item { Layout.fillHeight: true }
}
