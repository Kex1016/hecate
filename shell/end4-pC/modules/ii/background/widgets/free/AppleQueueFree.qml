pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.services.apple
import qs.modules.common.widgets.haiiro as H

// cider: up next, free · the song playing at 48px, its art floating on a 4px hard shadow with
// the rose » beside it, then the queue numbered at 24px. Without cover art in the queue the
// squares are the dither placeholder. With `items` ([{ type: "songs", id, name, artist,
// durationMs }], from a search or a Claude answer) it is a queue to look over: nothing plays
// until "play in cider". `prompt` is the question line it came from, `label` is
// "claude: queue" then. Cider's own queue gets "clear".
H.FreeWidget {
    id: root

    property var items: []
    property string prompt: ""
    property int maxRows: 3

    readonly property bool proposed: root.items.length > 0
    // a proposed queue starts with the song that would play first; Cider's own with the one playing
    readonly property var rows: root.proposed ? root.items.slice(1) : Cider.upNext
    readonly property var current: root.proposed ? root.items[0] : (Cider.open ? Cider.track : null)
    readonly property int hiddenCount: Math.max(0, root.rows.length - root.maxRows)
    readonly property real hiddenMs: root.rows.slice(root.maxRows).reduce((t, r) => t + (r.durationMs ?? 0), 0)
    readonly property bool hasCurrent: root.current !== null && root.current !== undefined
    readonly property string cover: !root.proposed && Cider.active && MprisController.activePlayer ? (MprisController.activePlayer.trackArtUrl ?? "") : ""

    signal played()

    label: root.proposed ? "claude: queue" : "cider: up next"
    mark: root.proposed ? "" : "♪"
    implicitWidth: 360
    spacing: 8

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

    // an `n` x `n` checker of muted on base, ink where x + y is odd
    function checker(n) {
        return Array.from({ length: n }, (_, y) => Array.from({ length: n }, (_, x) => (x + y) % 2 === 1 ? "m" : " ").join(""));
    }

    // the dither placeholder, or the cover when there is one
    component Cover: H.FreeArt {
        id: art
        property string source: ""
        // the checker is drawn on a 12 x 12 grid whatever the size
        readonly property int grid: 12
        Rectangle {
            anchors.fill: parent
            color: H.HTokens.base
        }
        H.PixelArt {
            visible: pic.status !== Image.Ready
            cell: art.width / art.grid
            shadow: false
            rows: root.checker(art.grid)
        }
        Image {
            id: pic
            anchors.fill: parent
            source: art.source
            sourceSize: Qt.size(art.width, art.height)
            fillMode: Image.PreserveAspectCrop
            smooth: false
            mipmap: false
            asynchronous: true
            visible: status === Image.Ready
        }
    }

    // the song playing
    RowLayout {
        visible: root.hasCurrent
        Layout.fillWidth: true
        spacing: 12
        H.FreeText { role: "code"; ink: "rose"; text: "»"; Layout.alignment: Qt.AlignTop; Layout.topMargin: 14 }
        Cover {
            width: 48
            height: 48
            Layout.preferredWidth: 48
            Layout.preferredHeight: 48
            offset: 4
            source: root.cover
        }
        ColumnLayout {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            spacing: 2
            H.FreeText {
                Layout.fillWidth: true
                role: "nav"
                elide: Text.ElideRight
                text: root.current?.name ?? ""
            }
            H.FreeText {
                Layout.fillWidth: true
                role: "code"
                ink: "c6"
                elide: Text.ElideRight
                text: root.current?.artist ?? ""
            }
        }
    }

    // the queue
    Repeater {
        model: root.rows.slice(0, root.maxRows)
        delegate: RowLayout {
            id: row
            required property var modelData
            required property int index
            Layout.fillWidth: true
            spacing: 12
            H.FreeText { role: "code"; ink: "subtle"; text: String(row.index + 1); Layout.preferredWidth: 6 }
            Cover { width: 24; height: 24; Layout.preferredWidth: 24; Layout.preferredHeight: 24; offset: 0 }
            H.FreeText {
                Layout.fillWidth: true
                role: "code"
                elide: Text.ElideRight
                textFormat: Text.StyledText
                text: row.modelData.name.replace(/&/g, "&amp;").replace(/</g, "&lt;")
                    + (row.modelData.artist ? ` <font color="${H.HTokens.foam}">${row.modelData.artist.replace(/&/g, "&amp;").replace(/</g, "&lt;")}</font>` : "")
            }
        }
    }
    H.FreeText {
        visible: root.hiddenCount > 0
        role: "caption"
        ink: "subtle"
        textFormat: Text.StyledText
        text: `+ ${root.hiddenCount} more` + (root.hiddenMs > 0 ? ` · ${root.duration(root.hiddenMs)}` : "")
            + (root.proposed && root.prompt ? ` · from <font color="${H.HTokens.gold}">?</font> ${root.prompt.replace(/&/g, "&amp;").replace(/</g, "&lt;")}` : "")
    }

    H.FreeText {
        Layout.fillWidth: true
        visible: root.rows.length === 0 && !root.hasCurrent
        role: "code"
        ink: "muted"
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
