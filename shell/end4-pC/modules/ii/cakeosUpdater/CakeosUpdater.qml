pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs
import qs.services
import qs.modules.common
import qs.modules.common.functions
import qs.modules.common.widgets

/**
 * hecate on CakeOS: the system updater. What's new upstream, when to apply it (next restart or
 * now), the password, then `update-cakeos apply` with its progress. The update itself runs in
 * the Updates service, so closing this doesn't stop it.
 */
Scope {
    id: root

    function relativeDate(iso) {
        const days = Math.floor((Date.now() - new Date(iso).getTime()) / 86400000);
        if (days <= 0) return Translation.tr("today");
        if (days === 1) return Translation.tr("yesterday");
        return Translation.tr("%1 days ago").arg(days);
    }

    IpcHandler {
        target: "cakeosUpdater"
        function open(): void {
            Updates.openUpdater();
        }
    }

    Loader {
        active: Updates.updaterOpen
        sourceComponent: PanelWindow {
            id: panel
            screen: Quickshell.screens.find(s => s.name === WM.focusedMonitor?.name) ?? Quickshell.screens[0]
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "quickshell:cakeosUpdater"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
            color: "transparent"
            anchors {
                top: true
                bottom: true
                left: true
                right: true
            }

            // overview -> password (the user's step), then progress once an update runs or ran.
            property string step: "overview"
            readonly property string page: Updates.applying || Updates.applyResult === "ok" || Updates.applyResult === "failed"
                ? "progress" : Updates.applyResult === "wrongPassword" ? "password" : step
            property string mode: "boot"
            readonly property var status: Updates.cakeosStatus

            function close() {
                Updates.updaterOpen = false;
            }

            WindowDialog {
                id: dialog
                anchors.fill: parent
                backgroundWidth: 600
                show: false
                focus: true
                Component.onCompleted: show = true
                onDismiss: panel.close()

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    MaterialSymbol {
                        iconSize: 26
                        text: Updates.applyResult === "ok" ? "check_circle"
                            : Updates.applyResult === "failed" ? "error" : "system_update_alt"
                        color: Updates.applyResult === "failed" ? Appearance.colors.colError : Appearance.colors.colSecondary
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0
                        WindowDialogTitle {
                            Layout.fillWidth: true
                            text: Translation.tr("CakeOS update")
                        }
                        StyledText {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            color: Appearance.colors.colSubtext
                            text: {
                                const s = panel.status;
                                if (Updates.checking && !s) return Translation.tr("Checking for updates…");
                                if (!s) return "";
                                if (s.error) return Translation.tr("Couldn't check for updates: %1").arg(s.error);
                                if (s.behind === 0) return Translation.tr("Up to date with %1").arg(s.branch);
                                if (s.behind === null) return Translation.tr("Installed version unknown · %1 is at %2").arg(s.branch).arg(s.latest.slice(0, 7));
                                return Translation.tr("%1 new changes on %2").arg(s.behind).arg(s.branch);
                            }
                        }
                    }
                }

                // Fixed height: WindowDialog sizes its card once, when it opens.
                Item {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 320

                    // What's new.
                    ColumnLayout {
                        anchors.fill: parent
                        visible: panel.page === "overview"
                        spacing: 12

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            radius: Appearance.rounding.normal
                            color: Appearance.colors.colLayer2
                            clip: true

                            StyledListView {
                                id: commits
                                anchors.fill: parent
                                anchors.margins: 8
                                spacing: 2
                                model: panel.status?.commits ?? []
                                delegate: RowLayout {
                                    id: commit
                                    required property var modelData
                                    width: commits.width
                                    spacing: 10
                                    StyledText {
                                        font.family: Appearance.font.family.monospace
                                        font.pixelSize: Appearance.font.pixelSize.smaller
                                        color: Appearance.colors.colPrimary
                                        text: commit.modelData.sha.slice(0, 7)
                                    }
                                    StyledText {
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                        color: Appearance.colors.colOnLayer2
                                        text: commit.modelData.subject
                                    }
                                    StyledText {
                                        font.pixelSize: Appearance.font.pixelSize.smaller
                                        color: Appearance.colors.colSubtext
                                        text: root.relativeDate(commit.modelData.date)
                                    }
                                }
                            }
                            StyledText {
                                anchors.centerIn: parent
                                visible: commits.count === 0
                                color: Appearance.colors.colSubtext
                                text: Updates.checking ? Translation.tr("Checking…") : Translation.tr("Nothing new upstream")
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            StyledText {
                                Layout.fillWidth: true
                                color: Appearance.colors.colOnLayer1
                                text: Translation.tr("Apply")
                            }
                            // M3 Expressive connected button group.
                            Row {
                                spacing: 2
                                SelectionGroupButton {
                                    leftmost: true
                                    buttonIcon: "restart_alt"
                                    buttonText: Translation.tr("On next restart")
                                    toggled: panel.mode === "boot"
                                    onClicked: panel.mode = "boot"
                                }
                                SelectionGroupButton {
                                    rightmost: true
                                    buttonIcon: "bolt"
                                    buttonText: Translation.tr("Now")
                                    toggled: panel.mode === "switch"
                                    onClicked: panel.mode = "switch"
                                }
                            }
                        }
                    }

                    // The password.
                    ColumnLayout {
                        anchors.fill: parent
                        visible: panel.page === "password"
                        spacing: 12
                        onVisibleChanged: if (visible) passwordField.forceActiveFocus()

                        Item { Layout.fillHeight: true }
                        WindowDialogParagraph {
                            Layout.fillWidth: true
                            text: panel.mode === "boot"
                                ? Translation.tr("CakeOS will download the update and build it; it takes over at the next restart. Enter your password to continue.")
                                : Translation.tr("CakeOS will download the update, build it and switch to it right away. Enter your password to continue.")
                        }
                        MaterialTextField {
                            id: passwordField
                            Layout.fillWidth: true
                            placeholderText: Translation.tr("Password")
                            echoMode: TextInput.Password
                            onAccepted: if (text !== "") {
                                Updates.apply(text, panel.mode);
                                text = "";
                            }
                        }
                        StyledText {
                            visible: Updates.applyResult === "wrongPassword"
                            color: Appearance.colors.colError
                            text: Translation.tr("Wrong password")
                        }
                        Item { Layout.fillHeight: true }
                    }

                    // Progress and log.
                    ColumnLayout {
                        anchors.fill: parent
                        visible: panel.page === "progress"
                        spacing: 10

                        StyledText {
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            font.weight: Font.Medium
                            color: Updates.applyResult === "failed" ? Appearance.colors.colError : Appearance.colors.colOnLayer1
                            text: Updates.applyStatus
                        }
                        StyledIndeterminateProgressBar {
                            Layout.fillWidth: true
                            visible: Updates.applying
                        }
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            radius: Appearance.rounding.normal
                            color: Appearance.colors.colLayer2
                            clip: true

                            StyledListView {
                                id: log
                                anchors.fill: parent
                                anchors.margins: 8
                                model: Updates.applyLog
                                onCountChanged: positionViewAtEnd()
                                delegate: StyledText {
                                    required property string modelData
                                    width: log.width
                                    wrapMode: Text.WrapAnywhere
                                    font.family: Appearance.font.family.monospace
                                    font.pixelSize: Appearance.font.pixelSize.smaller
                                    color: modelData.startsWith("==> ") ? Appearance.colors.colPrimary
                                        : modelData.startsWith("!!! ") ? Appearance.colors.colError : Appearance.colors.colOnLayer2
                                    text: modelData
                                }
                            }
                        }
                    }
                }

                WindowDialogButtonRow {
                    Layout.bottomMargin: 10

                    DialogButton {
                        visible: panel.page === "overview"
                        buttonText: Translation.tr("Check again")
                        enabled: !Updates.checking
                        onClicked: Updates.refresh()
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                    DialogButton {
                        buttonText: Updates.applying ? Translation.tr("Hide")
                            : Updates.applyResult === "ok" && Updates.applyMode === "boot" ? Translation.tr("Later")
                            : panel.page === "progress" ? Translation.tr("Close") : Translation.tr("Cancel")
                        onClicked: {
                            if (panel.page === "password" && !Updates.applying) {
                                Updates.applyResult = "";
                                panel.step = "overview";
                            } else {
                                panel.close();
                            }
                        }
                    }
                    DialogButton {
                        visible: panel.page === "overview"
                        buttonText: panel.status?.behind === 0 ? Translation.tr("Reinstall") : Translation.tr("Update")
                        enabled: !Updates.checking
                        onClicked: panel.step = "password"
                    }
                    DialogButton {
                        visible: panel.page === "password"
                        buttonText: Translation.tr("Continue")
                        enabled: passwordField.text !== ""
                        onClicked: passwordField.accepted()
                    }
                    DialogButton {
                        visible: Updates.applyResult === "ok" && Updates.applyMode === "boot"
                        buttonText: Translation.tr("Restart now")
                        onClicked: Session.reboot()
                    }
                }
            }
        }
    }
}
