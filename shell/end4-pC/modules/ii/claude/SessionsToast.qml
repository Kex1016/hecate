pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H
import "SessionsFormat.js" as F

// One Claude toast, a notify window in the kit:
//
//   permission  gold frame. `~/src/cakeos wants to run nix build`, the whole
//               command under it, and allow, always in this repo, deny, look.
//               It stays until it is answered.
//   done        foam frame. `~/src/hecate-greetd is done.` and the first line
//               of what it says it did. It goes by itself, or on a click.
//
// The toast holds a snapshot of the session, so the close frames still have
// something to draw after the session has moved on.
H.PixelTransition {
    id: root

    required property string kind
    required property string repo
    required property string verb
    required property string subject
    required property string command
    required property string summary
    required property string error
    required property bool canAlways

    readonly property bool permission: kind === "permission"
    readonly property bool failed: !permission && error.length > 0
    readonly property var clipped: F.clip(command, 240)

    signal allow()
    signal always()
    signal deny()
    signal look()
    signal dismissed()

    width: H.HTokens.toastWidth
    height: window.implicitHeight

    H.HWindow {
        id: window
        width: root.width
        label: "notify: claude"
        variant: root.permission ? "gold" : root.failed ? "love" : "foam"

        MouseArea {
            Layout.fillWidth: true
            Layout.preferredHeight: col.implicitHeight
            enabled: !root.permission
            onClicked: root.dismissed()

            ColumnLayout {
                id: col
                width: parent.width
                spacing: 6

                H.HText {
                    Layout.fillWidth: true
                    textFormat: Text.StyledText
                    wrapMode: Text.Wrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                    text: root.permission
                        ? "<font color=\"" + H.HTokens.foam + "\">" + F.esc(root.repo) + "</font> wants to " + F.esc(root.verb) + " <font color=\"" + H.HTokens.gold + "\">" + F.esc(root.subject) + "</font>"
                        : "<font color=\"" + H.HTokens.foam + "\">" + F.esc(root.repo) + "</font>" + (root.failed ? " <font color=\"" + H.HTokens.love + "\">stopped</font>." : " is done.")
                }

                H.CodeBlock {
                    visible: root.permission && root.clipped.text.length > 0 && root.clipped.text !== root.subject
                    Layout.fillWidth: true
                    lines: [root.clipped.text]
                }
                H.HText {
                    visible: root.permission && root.clipped.more > 0
                    Layout.fillWidth: true
                    text: "+" + root.clipped.more + " more characters, see the whole command in the panel (Mod+Shift+/)"
                    role: "caption"
                    ink: "c3"
                    wrapMode: Text.Wrap
                }
                H.HText {
                    visible: !root.permission && (root.summary.length > 0 || root.failed)
                    Layout.fillWidth: true
                    text: root.failed ? root.error : root.summary
                    role: "code"
                    ink: root.failed ? "c2" : "c1"
                    wrapMode: Text.Wrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                }

                RowLayout {
                    visible: root.permission
                    Layout.fillWidth: true
                    spacing: 8

                    H.InkButton { text: "allow"; ink: "c3"; onClicked: root.allow() }
                    H.InkButton { visible: root.canAlways; text: "always in this repo"; ink: "c5"; onClicked: root.always() }
                    H.InkButton { text: "deny"; ink: "c2"; onClicked: root.deny() }
                    H.InkButton { text: "look"; ink: "c1"; onClicked: root.look() }
                }
            }
        }
    }
}
