import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro

// A row in a launcher, clipboard or picker list. The selected row is an
// overlay band with a rose » in front; other rows keep the marker's space.
// `match` paints the part of `text` that matched the search in foam.
// Children go after `meta` (a Keycap for the selected row's `enter`);
// `leading` takes items before the text (a thumbnail, a dithered icon).
Item {
    id: root

    property string text: ""
    property string meta: ""
    property string match: ""
    property string ink: "text"
    property bool selected: false
    property alias leading: lead.data
    default property alias trailing: trail.data
    readonly property bool hovered: mouse.containsMouse
    signal clicked()
    signal entered()

    implicitWidth: row.implicitWidth + 16
    implicitHeight: Math.max(28, row.implicitHeight) + 8

    function esc(s) {
        return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    }
    readonly property string styled: {
        const t = root.text;
        const i = root.match.length > 0 ? t.toLowerCase().indexOf(root.match.toLowerCase()) : -1;
        if (i < 0) return esc(t);
        return esc(t.slice(0, i)) + "<font color=\"" + HTokens.foam + "\">" + esc(t.slice(i, i + root.match.length)) + "</font>" + esc(t.slice(i + root.match.length));
    }

    Rectangle {
        anchors.fill: parent
        visible: root.selected
        color: HTokens.overlay
    }

    RowLayout {
        id: row
        x: 8
        width: parent.width - 16
        anchors.verticalCenter: parent.verticalCenter
        spacing: 12

        HText {
            Layout.preferredWidth: HTokens.ch()
            text: "»"
            ink: "rose"
            opacity: root.selected ? 1 : 0
        }
        Row {
            id: lead
            visible: children.length > 0
            spacing: 8
        }
        HText {
            Layout.fillWidth: true
            text: root.styled
            textFormat: Text.StyledText
            ink: root.ink
            elide: Text.ElideRight
            maximumLineCount: 1
        }
        HText {
            visible: root.meta.length > 0
            text: root.meta
            role: "caption"
            ink: "inkMuted"
        }
        Row {
            id: trail
            visible: children.length > 0
            spacing: 8
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        onClicked: root.clicked()
        onEntered: root.entered()
    }
}
