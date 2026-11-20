pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services.apple
import qs.modules.common.widgets.haiiro as H

// A queue you look over before anything plays.
//
// With `items` empty it is Cider's own up next (services/apple/Cider.qml), and its button
// clears it. Given `items` ([{ type: "songs", id, name, artist, durationMs }], from a search
// with Cider.searchSongs, or a Claude answer), nothing plays until "play in cider": then the
// first replaces what is playing and the rest follow in order. `prompt` is the question
// line above the list, `label` the window's name ("claude: queue" on the board).
H.HWindow {
    id: root

    property bool live: true
    property var items: []
    property string prompt: ""
    property int maxRows: 2

    readonly property bool proposed: root.items.length > 0
    readonly property var rows: root.proposed ? root.items : Cider.upNext
    readonly property int hiddenCount: Math.max(0, root.rows.length - root.maxRows)
    readonly property real hiddenMs: root.rows.slice(root.maxRows).reduce((t, r) => t + (r.durationMs ?? 0), 0)

    signal played()

    label: root.proposed ? "claude: queue" : "cider: up next"
    widget: true
    implicitWidth: 360

    property bool held: false
    function hold() {
        if (root.live && !root.held) { Cider.consumers++; root.held = true; }
        else if (!root.live && root.held) { Cider.consumers--; root.held = false; }
    }
    onLiveChanged: hold()
    Component.onCompleted: hold()
    Component.onDestruction: if (root.held) Cider.consumers--

    function duration(ms) {
        const minutes = Math.round(ms / 60000);
        const h = Math.floor(minutes / 60);
        return h > 0 ? `${h}h ${minutes % 60}m` : `${minutes}m`;
    }

    // ? play something for focus
    RowLayout {
        visible: root.prompt.length > 0
        spacing: H.HTokens.ch(14)
        H.HText { role: "code"; ink: "c3"; text: "?" }
        H.HText { Layout.fillWidth: true; role: "code"; elide: Text.ElideRight; text: root.prompt }
    }

    Repeater {
        model: root.rows.slice(0, root.maxRows)
        delegate: H.HText {
            required property var modelData
            Layout.fillWidth: true
            role: "code"
            elide: Text.ElideRight
            text: `» ${modelData.artist ? modelData.artist + " · " : ""}${modelData.name}`
        }
    }
    H.HText {
        visible: root.hiddenCount > 0
        role: "code"
        ink: "c1"
        text: `» + ${root.hiddenCount} more` + (root.hiddenMs > 0 ? `, ${root.duration(root.hiddenMs)}` : "")
    }

    H.HText {
        Layout.fillWidth: true
        visible: root.rows.length === 0
        role: "code"
        ink: "c1"
        wrapMode: Text.Wrap
        text: !Cider.open ? "open cider to see its queue" : "nothing up next"
    }

    RowLayout {
        visible: root.proposed || root.rows.length > 0
        spacing: 8
        H.InkButton {
            visible: root.proposed
            ink: "c3"
            role: "code"
            text: "play in cider"
            enabled: Cider.open
            onClicked: {
                Cider.playItems(root.items);
                root.played();
            }
        }
        H.InkButton {
            visible: !root.proposed
            ink: "c1"
            role: "code"
            text: "clear"
            onClicked: Cider.clearQueue()
        }
    }
}
