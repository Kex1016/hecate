import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

/**
 * hecate: asks for the sudo password, then runs the boot-to-Windows script
 * (Config.options.apps.bootWindows) as root. The script sets the next EFI boot entry and reboots.
 */
Item {
    id: root
    signal closed()

    property string error: ""
    property string password: ""
    readonly property bool busy: bootProc.running
    readonly property string script: Config.options.apps.bootWindows.replace(/^~(?=\/|$)/, Quickshell.env("HOME"))

    function submit() {
        if (root.busy || line.text === "") return;
        root.error = "";
        root.password = line.text;
        line.clear();
        bootProc.running = true;
    }
    function cancel() {
        if (!root.busy) root.closed();
    }

    Keys.onEscapePressed: root.cancel()

    Process {
        id: bootProc
        // -k: always check the password; env keeps the user's PATH (nu, efibootmgr) for the script.
        command: ["sudo", "-S", "-k", "-p", "", "env", `PATH=${Quickshell.env("PATH")}`, root.script]
        stdinEnabled: true
        onStarted: {
            bootProc.write(root.password + "\n");
            root.password = "";
            bootProc.stdinEnabled = false;
        }
        stderr: StdioCollector { id: stderrCollector }
        stdout: StdioCollector { id: stdoutCollector }
        onExited: (exitCode, exitStatus) => {
            bootProc.stdinEnabled = true;
            if (exitCode === 0) return; // Rebooting.
            const out = (stderrCollector.text + "\n" + stdoutCollector.text).trim();
            if (/incorrect password|try again|no password was provided/i.test(out))
                root.error = "Wrong password.";
            else if (/command not found|No such file/i.test(out))
                root.error = `${Config.options.apps.bootWindows} doesn't exist.`;
            else
                root.error = out.split("\n").filter(l => l.trim() !== "").pop() ?? `Failed (exit code ${exitCode}).`;
            line.forceInputFocus();
        }
    }

    // dims the menu behind; a click outside the window cancels
    H.Veil {
        anchors.fill: parent
        onClicked: root.cancel()
    }

    H.PixelTransition {
        id: pixels
        anchors.centerIn: parent
        width: win.implicitWidth
        height: win.implicitHeight
        H.HWindow {
            id: win
            width: 540
            height: implicitHeight
            label: "sudo: boot-windows"
            hints: [
                { key: "enter", text: root.busy ? "working" : "reboot into windows" },
                { key: "esc", text: "cancel" }
            ]
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 12
                H.HText {
                    Layout.fillWidth: true
                    text: root.busy ? "The other drive hums. Hold on." : "You reach for the other drive. It asks who you are."
                    ink: "rose"
                    wrapMode: Text.Wrap
                }
                RowLayout {
                    spacing: 0
                    H.HText { text: "Next boot goes to " }
                    H.HText { text: "Windows Boot Manager"; ink: "iris" }
                    H.HText { text: ", once." }
                }
                PasswordLine {
                    id: line
                    busy: root.busy
                    onAccepted: root.submit()
                    onCancelled: root.cancel()
                    H.HText { text: "[sudo]"; ink: "gold" }
                    H.HText { text: "password for" }
                    H.HText { text: Quickshell.env("USER") + ":"; ink: "pine" }
                }
                H.HText {
                    Layout.fillWidth: true
                    visible: root.error !== ""
                    text: root.error
                    ink: "love"
                    wrapMode: Text.Wrap
                }
                RowLayout {
                    spacing: 8
                    H.InkButton {
                        text: "reboot into windows"
                        ink: "c7"
                        enabled: !root.busy && line.text !== ""
                        onClicked: root.submit()
                    }
                    H.InkButton {
                        text: "cancel"
                        ink: "c1"
                        enabled: !root.busy
                        onClicked: root.cancel()
                    }
                }
            }
        }
    }

    Component.onCompleted: line.forceInputFocus()
}
