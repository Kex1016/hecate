import QtQuick
import qs.modules.common.widgets.haiiro

// A key name in a 1px ink-muted box: `Mod+A`, `enter`, `ctrl enter`.
// `hint` is what the key does, written after it; `quiet` sets the hint in
// the footer style (12px ink-muted), which HWindow's footer uses.
Row {
    id: root

    property string key: ""
    property string hint: ""
    property bool quiet: false

    spacing: hintText.visible ? HTokens.ch(quiet ? HTokens.fontLabel : HTokens.fontBody) : 0

    Rectangle {
        anchors.verticalCenter: parent.verticalCenter
        width: keyText.implicitWidth + 8 + 2 * HTokens.borderHairline
        height: 16 + 2 * HTokens.borderHairline
        color: "transparent"
        border.width: HTokens.borderHairline
        border.color: HTokens.inkMuted

        HText {
            id: keyText
            x: 4 + HTokens.borderHairline
            y: HTokens.borderHairline
            height: 16
            lineHeight: 16
            text: root.key
            role: "caption"
            ink: "subtle"
        }
    }

    HText {
        id: hintText
        anchors.verticalCenter: parent.verticalCenter
        visible: root.hint.length > 0
        text: root.hint
        role: root.quiet ? "caption" : "body"
        ink: root.quiet ? "inkMuted" : "text"
    }
}
