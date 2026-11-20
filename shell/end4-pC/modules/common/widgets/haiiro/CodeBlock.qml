pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// Commands or a diff on base inside a window, 14px. Each line of `lines` is
// coloured the PromptLine way: a line starting "- " is love (removed), "+ "
// foam (added), "# " muted (a comment); otherwise the first word is the
// command in gold and the rest are arguments in iris. A command shown here
// never runs until a gold InkButton is pressed.
//   CodeBlock { lines: ["mkdir -p gen", "- bar.clock.seconds = false", "+ bar.clock.seconds = true"] }
// Children are added after the lines, for anything custom.
Rectangle {
    id: root

    property var lines: []
    default property alias extra: col.data

    color: HTokens.base
    implicitWidth: col.implicitWidth + 20
    implicitHeight: col.implicitHeight + 12

    function esc(s) {
        return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/ /g, "&nbsp;");
    }
    function styled(line) {
        const paint = (c, s) => "<font color=\"" + c + "\">" + esc(s) + "</font>";
        if (line.startsWith("- ") || line === "-") return paint(HTokens.love, line);
        if (line.startsWith("+ ") || line === "+") return paint(HTokens.foam, line);
        if (line.startsWith("# ")) return paint(HTokens.muted, line);
        const sp = line.indexOf(" ");
        if (sp < 0) return paint(HTokens.gold, line);
        return paint(HTokens.gold, line.slice(0, sp)) + "&nbsp;" + paint(HTokens.iris, line.slice(sp + 1));
    }

    Column {
        id: col
        x: 10
        y: 6
        width: root.width - 20
        spacing: 2

        Repeater {
            model: root.lines
            HText {
                required property string modelData
                width: col.width
                text: root.styled(modelData)
                textFormat: Text.StyledText
                role: "code"
                wrapMode: Text.WrapAnywhere
            }
        }
    }
}
