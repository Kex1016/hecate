pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H
import "SessionsFormat.js" as F

// One Claude Code session in the sessions panel, as the ClaudeSessions board
// draws it: a 1px box (gold while it waits for you) with the repo and its
// state on the first line, then what the state needs.
//
//   waiting  the command it wants to run, and the answers
//   working  what it is doing now, on a stepped bar
//   done     what it says it did, and the way to its diff
//
// The card only reports presses; the panel decides what they do.
Item {
    id: root

    required property var session
    property bool selected: false
    property double now: Date.now()
    // "kitty · ws 3": where the session's terminal is, when it is known
    property string place: ""
    property bool answersEnabled: true

    signal allow()
    signal always()
    signal deny()
    signal goToTerminal()
    signal showDiff()
    signal dismiss()

    readonly property string state: session.state ?? "working"
    readonly property bool waiting: state === "waiting"
    readonly property bool done: state === "done"
    readonly property bool idle: state === "idle"
    // what a waiting session asks, when it asks to run something; a session can also
    // wait for an answer in its terminal, which only the terminal can give
    readonly property var request: session.request ?? null
    readonly property bool failed: done && (session.error ?? "").length > 0
    readonly property color stateColor: waiting ? H.HTokens.gold : done ? H.HTokens.muted : H.HTokens.pine

    implicitWidth: 560
    implicitHeight: col.implicitHeight + 16

    Rectangle {
        anchors.fill: parent
        color: "transparent"
        border.width: H.HTokens.borderHairline
        border.color: root.waiting ? H.HTokens.gold : root.selected ? H.HTokens.subtle : H.HTokens.inkHairline
    }

    ColumnLayout {
        id: col
        x: 10
        y: 8
        width: root.width - 20
        spacing: 6

        // repo and state, then where it is or how long ago
        RowLayout {
            Layout.fillWidth: true
            spacing: H.HTokens.ch()

            H.HText {
                visible: root.selected
                text: "»"
                ink: "c4"
            }
            H.HText {
                text: root.session.dir || root.session.cwd
                ink: "c6"
                elide: Text.ElideMiddle
                Layout.maximumWidth: 300
            }
            H.HText {
                text: root.waiting ? "waiting for you" : root.failed ? "stopped" : root.state
                ink: root.waiting ? "c3" : root.failed ? "c2" : root.done || root.idle ? "c1" : "c5"
            }
            Item { Layout.fillWidth: true }
            H.HText {
                text: root.done ? F.age(root.session.doneAt || root.session.updatedAt || root.now, root.now) + " ago"
                    : (root.state === "working" ? F.span(root.session.turnStartedAt || root.session.startedAt || root.now, root.now) + (root.place ? " · " : "") : "") + root.place
                role: "caption"
                ink: "c1"
            }
        }

        // ---- waiting ----
        H.HText {
            visible: root.waiting
            Layout.fillWidth: true
            text: root.request ? "wants to " + (root.request.verb || "use") + ":" : (root.session.message || "is asking you something in its terminal.")
            wrapMode: Text.Wrap
            maximumLineCount: 3
            elide: Text.ElideRight
        }
        H.CodeBlock {
            visible: root.waiting && root.request !== null
            Layout.fillWidth: true
            lines: root.request ? [F.clip(root.request.detail || root.request.subject || root.request.tool, 600).text] : []
        }
        H.HText {
            visible: root.waiting && root.request !== null && F.clip(root.request?.detail, 600).more > 0
            Layout.fillWidth: true
            text: "+" + F.clip(root.request?.detail, 600).more + " more characters: answer in the terminal to see all of it"
            role: "caption"
            ink: "c3"
            wrapMode: Text.Wrap
        }
        RowLayout {
            visible: root.waiting
            Layout.fillWidth: true
            spacing: 8

            H.InkButton { visible: root.request !== null; text: "allow once"; ink: "c3"; enabled: root.answersEnabled; onClicked: root.allow() }
            H.InkButton { visible: root.request !== null && root.request.canAlways === true; text: "always in this repo"; ink: "c5"; enabled: root.answersEnabled; onClicked: root.always() }
            H.InkButton { visible: root.request !== null; text: "deny"; ink: "c2"; enabled: root.answersEnabled; onClicked: root.deny() }
            H.InkButton { text: "go to terminal"; flat: root.request !== null; ink: root.request !== null ? "" : "c3"; onClicked: root.goToTerminal() }
        }
        // what "always" would do, as Claude Code worded the rule
        H.HText {
            visible: root.waiting && root.request !== null && root.request.canAlways === true && (root.request.alwaysLabel ?? "").length > 0
            Layout.fillWidth: true
            text: "always = " + (root.request?.alwaysLabel ?? "")
            role: "caption"
            ink: "c1"
            elide: Text.ElideRight
        }

        // ---- working ----
        H.HText {
            visible: root.state === "working"
            Layout.fillWidth: true
            text: "» " + (root.session.activity || "thinking")
            role: "code"
            ink: "c1"
            elide: Text.ElideRight
        }
        H.Progress {
            id: bar
            visible: root.state === "working"
            Layout.fillWidth: true
            // a session reports a fraction when it has one (a todo list);
            // otherwise the bar steps along so you can see it is alive
            property int tick: 0
            value: (root.session.progress ?? -1) >= 0 ? root.session.progress : (tick % 11) / 10
            Timer {
                running: bar.visible && (root.session.progress ?? -1) < 0
                interval: 500
                repeat: true
                onTriggered: bar.tick += 1
            }
        }

        // ---- done ----
        H.HText {
            visible: root.done
            Layout.fillWidth: true
            text: root.failed ? "stopped: " + root.session.error : (root.session.summary || "Finished.")
            ink: root.failed ? "c2" : "text"
            role: "code"
            wrapMode: Text.Wrap
            maximumLineCount: 3
            elide: Text.ElideRight
        }
        RowLayout {
            visible: root.done
            Layout.fillWidth: true
            spacing: 8

            H.InkButton { text: "show diff"; ink: "c1"; onClicked: root.showDiff() }
            H.InkButton { text: "dismiss"; flat: true; onClicked: root.dismiss() }
        }

        // ---- idle: started, nothing asked yet ----
        RowLayout {
            visible: root.idle
            Layout.fillWidth: true
            spacing: 8

            H.InkButton { text: "go to terminal"; ink: "c1"; onClicked: root.goToTerminal() }
            H.InkButton { visible: root.session.ended === true; text: "dismiss"; flat: true; onClicked: root.dismiss() }
        }
    }
}
