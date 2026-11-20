pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// `man launch`: the prefixes, two to a line, under the launcher while
// nothing has switched it into a mode. `?` stays out until Claude is wired.
H.HWindow {
    id: root

    label: "man launch"
    width: 720

    GridLayout {
        Layout.fillWidth: true
        columns: 4
        columnSpacing: 12
        rowSpacing: 6

        Repeater {
            model: LauncherSearch.prefixes.filter(p => p.mode !== "claude" || LauncherSearch.claudeReady)

            Item {
                id: entry
                required property var modelData
                required property int index
                Layout.columnSpan: 2
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                implicitHeight: desc.implicitHeight

                H.HText {
                    id: token
                    width: 6 * H.HTokens.ch()
                    text: entry.modelData.token.trim()
                    ink: "gold"
                }
                H.HText {
                    id: desc
                    x: token.width + 12
                    width: parent.width - x
                    textFormat: Text.StyledText
                    elide: Text.ElideRight
                    text: entry.modelData.text + (entry.modelData.note
                        ? " <font color=\"" + H.HTokens.muted + "\">" + entry.modelData.note + "</font>" : "")
                }
            }
        }
    }
}
