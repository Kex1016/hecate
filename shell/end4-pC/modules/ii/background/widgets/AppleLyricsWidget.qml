pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services.apple
import qs.modules.common.widgets.haiiro as H

// media: lyrics · the line being sung in rose after a », the one before and after it
// muted, and the heart that adds the track to your Apple Music library. From Cider
// (services/apple/Cider.qml): Apple Music's synced lyrics, through Cider's session.
// Meant for the media panel (Mod+M) and the bar's media hover; it works on the desktop too.
H.HWindow {
    id: root

    property bool live: true
    // Lines shown before and after the current one.
    property int around: 1

    label: "media: lyrics"
    widget: true
    implicitWidth: 360

    // Faster polling and the line timer while this is up.
    property bool held: false
    function hold() {
        if (root.live && !root.held) { Cider.consumers++; root.held = true; }
        else if (!root.live && root.held) { Cider.consumers--; root.held = false; }
    }
    onLiveChanged: hold()
    Component.onCompleted: hold()
    Component.onDestruction: if (root.held) Cider.consumers--

    readonly property bool showLines: Cider.active && Cider.hasLines

    Repeater {
        model: root.showLines ? root.around * 2 + 1 : 0
        delegate: H.HText {
            id: line
            required property int index
            readonly property int offset: index - root.around
            readonly property int lineIndex: Math.max(0, Cider.lineIndex) + offset
            readonly property bool current: offset === 0 && Cider.lineIndex >= 0
            Layout.fillWidth: true
            role: "code"
            ink: line.current ? "c4" : "c1"
            elide: Text.ElideRight
            text: (line.current ? "» " : "") + Cider.lineAt(line.lineIndex)
        }
    }

    H.HText {
        Layout.fillWidth: true
        visible: !root.showLines
        role: "code"
        ink: "c1"
        wrapMode: Text.Wrap
        text: !Cider.open ? "open cider for lyrics"
            : Cider.status === "off" ? "turn on cider's rpc server: settings › connectivity"
            : Cider.status === "token" ? "cider wants a token: apple.cider.token, or the keyring"
            : !Cider.active ? "cider is not the player shown"
            : Cider.lyricsStatus === "loading" ? "…"
            : "no lyrics for this one"
    }

    RowLayout {
        visible: Cider.open && Cider.reachable && Cider.track !== null
        spacing: 8
        H.InkButton {
            ink: Cider.inLibrary ? "c1" : "c3"
            role: "code"
            text: Cider.inLibrary ? "♥ in library" : "♥ add to library"
            enabled: !Cider.inLibrary
            onClicked: Cider.addToLibrary()
        }
        H.HText {
            Layout.alignment: Qt.AlignVCenter
            role: "caption"
            ink: "c1"
            text: "via cider"
        }
    }
}
