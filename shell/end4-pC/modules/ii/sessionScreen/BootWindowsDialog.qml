import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.services
import qs.modules.common.widgets

/**
 * hecate: asks for the sudo password, then runs the boot-to-Windows script
 * (Config.options.apps.bootWindows) as root. The script sets the next EFI boot entry and reboots.
 */
Item {
    id: root
    signal closed()

    property string error: ""
    readonly property bool busy: bootProc.running
    readonly property string script: Config.options.apps.bootWindows.replace(/^~(?=\/|$)/, Quickshell.env("HOME"))

    function submit() {
        if (root.busy || inputField.text === "") return;
        root.error = "";
        root.password = inputField.text;
        inputField.text = "";
        bootProc.running = true;
    }
    function cancel() {
        if (!root.busy) root.closed();
    }
    property string password: ""

    Keys.onPressed: event => {
        if (event.key === Qt.Key_Escape) root.cancel();
    }

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
                root.error = Translation.tr("Wrong password");
            else if (/command not found|No such file/i.test(out))
                root.error = Translation.tr("%1 doesn't exist").arg(Config.options.apps.bootWindows);
            else
                root.error = out.split("\n").filter(l => l.trim() !== "").pop() ?? Translation.tr("Failed (exit code %1)").arg(exitCode);
            inputField.forceActiveFocus();
        }
    }

    Rectangle {
        anchors.fill: parent
        color: Appearance.colors.colScrim
        MouseArea {
            anchors.fill: parent
            onClicked: root.cancel()
        }
    }

    WindowDialog {
        anchors.centerIn: parent
        backgroundWidth: 450
        show: false
        Component.onCompleted: {
            show = true;
            inputField.forceActiveFocus();
        }

        MaterialSymbol {
            Layout.alignment: Qt.AlignHCenter
            iconSize: 26
            text: "window"
            color: Appearance.colors.colSecondary
        }

        WindowDialogTitle {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            text: Translation.tr("Boot Windows")
        }

        WindowDialogParagraph {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignLeft
            text: root.busy
                ? Translation.tr("Rebooting into Windows…")
                : Translation.tr("The computer restarts into Windows once. Enter your password to continue.")
        }

        MaterialTextField {
            id: inputField
            Layout.fillWidth: true
            focus: true
            enabled: !root.busy
            placeholderText: Translation.tr("Password")
            echoMode: TextInput.Password
            onAccepted: root.submit()
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Escape) root.cancel();
            }
        }

        StyledText {
            Layout.fillWidth: true
            visible: root.error !== ""
            wrapMode: Text.Wrap
            color: Appearance.colors.colError
            text: root.error
        }

        WindowDialogButtonRow {
            Layout.bottomMargin: 10
            Item {
                Layout.fillWidth: true
            }
            DialogButton {
                buttonText: Translation.tr("Cancel")
                enabled: !root.busy
                onClicked: root.cancel()
            }
            DialogButton {
                buttonText: Translation.tr("Reboot")
                enabled: !root.busy && inputField.text !== ""
                onClicked: root.submit()
            }
        }
    }
}
