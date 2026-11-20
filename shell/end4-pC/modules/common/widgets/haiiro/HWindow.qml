pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro

// The terminal pane every panel, popup and toast lives in: surface fill, a
// 5px double frame (2px line, 1px gap, 2px line), square corners and the
// label notched into the top edge (`notify: discord`, `claude: ~/ask`).
//
//   HWindow {
//       label: "cat desktop.toml"
//       variant: "gold"              // pine (default), love, gold, foam
//       hints: [{ key: "enter", text: "go" }, { key: "esc", text: "close" }]
//       note: "saved to ~/.config/hecate/shell.toml"
//       HText { text: "..." }        // children stack as lines
//   }
Item {
    id: root

    property string label: ""
    // pine: everything; love: critical or failed; gold: needs you; foam: done.
    property string variant: "pine"
    // Desktop widgets: lines get an 8px gap instead of none.
    property bool widget: false
    // Footer: key hints on the left, what the window saves to or sends on the right.
    property var hints: []
    property string note: ""
    property alias footer: footExtra.data
    default property alias content: body.data
    property alias body: body
    readonly property color frameColor: variant === "love" ? HTokens.love : variant === "gold" ? HTokens.gold : variant === "foam" ? HTokens.foam : HTokens.pine

    readonly property int frame: HTokens.borderWindow
    readonly property int pad: 5
    readonly property int inset: frame + pad + 10
    readonly property bool hasFooter: hints.length > 0 || note.length > 0 || footExtra.children.length > 0

    implicitWidth: Math.max(body.implicitWidth, hasFooter ? foot.implicitWidth : 0) + 2 * inset
    implicitHeight: inset + body.implicitHeight + 10 + (hasFooter ? foot.implicitHeight : 0) + frame + pad

    // fill, including the gap between the two frame lines
    Rectangle {
        anchors.fill: parent
        color: HTokens.surface
    }

    // the double frame: two 2px rings, 1px apart; colour changes snap
    Repeater {
        model: [0, 3]
        Rectangle {
            required property int modelData
            x: modelData
            y: modelData
            width: root.width - 2 * modelData
            height: root.height - 2 * modelData
            color: "transparent"
            border.width: 2
            border.color: root.frameColor
        }
    }

    // the notch: label on surface, centred on the frame's inner edge
    Rectangle {
        visible: root.label.length > 0
        x: root.frame + 15
        y: root.frame - Math.round(height / 2)
        width: labelText.implicitWidth + 2 * HTokens.ch()
        height: labelText.implicitHeight
        color: HTokens.surface

        HText {
            id: labelText
            x: HTokens.ch()
            text: root.label
        }
    }

    ColumnLayout {
        id: body
        x: root.inset
        y: root.inset
        width: root.width - 2 * root.inset
        height: root.height - y - 10 - (root.hasFooter ? foot.height : 0) - root.frame - root.pad
        spacing: root.widget ? 8 : 0
        clip: height < implicitHeight
    }

    Item {
        id: foot
        visible: root.hasFooter
        x: root.frame + root.pad
        width: root.width - 2 * x
        y: root.height - root.frame - root.pad - height
        implicitWidth: footRow.implicitWidth + 20
        implicitHeight: 1 + 6 + footRow.implicitHeight + 2
        height: implicitHeight

        Rectangle {
            width: parent.width
            height: HTokens.borderHairline
            color: HTokens.inkHairline
        }

        RowLayout {
            id: footRow
            x: 10
            y: 7
            width: parent.width - 20
            spacing: 16

            Repeater {
                model: root.hints
                Keycap {
                    required property var modelData
                    key: modelData.key
                    hint: modelData.text ?? ""
                    quiet: true
                }
            }
            Row {
                id: footExtra
                spacing: 16
            }
            Item { Layout.fillWidth: true }
            HText {
                visible: root.note.length > 0
                text: root.note
                role: "caption"
                ink: "inkMuted"
            }
        }
    }
}
