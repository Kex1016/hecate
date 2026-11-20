pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs
import qs.services
import qs.modules.common.functions
import qs.modules.common.widgets.haiiro as H

/**
 * hecate on CakeOS: the system updater, in the haiiro style (the canvas's Updater board). What
 * is new upstream as a code block, when to apply it (next boot or now), the password at a
 * prompt, then `update-cakeos apply` with its stages and log. The update itself runs in the
 * Updates service, so closing this doesn't stop it.
 */
Scope {
    id: root

    // The window pixelates out after Updates.updaterOpen goes false, so the surface stays
    // mapped until the transition says it is gone.
    property bool lingering: false

    // "2h", "5d": how long ago a commit landed.
    function ago(iso) {
        const minutes = Math.max(0, Math.floor((Date.now() - new Date(iso).getTime()) / 60000));
        if (minutes < 60) return `${minutes}m`;
        if (minutes < 60 * 24) return `${Math.floor(minutes / 60)}h`;
        return `${Math.floor(minutes / (60 * 24))}d`;
    }

    function esc(s) {
        return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    }

    IpcHandler {
        target: "cakeosUpdater"
        function open(): void {
            Updates.openUpdater();
        }
    }

    Connections {
        target: Updates
        function onUpdaterOpenChanged() {
            if (!Updates.updaterOpen) root.lingering = !H.HTokens.cut;
        }
    }

    Loader {
        active: Updates.updaterOpen || root.lingering
        sourceComponent: PanelWindow {
            id: panel
            screen: Quickshell.screens.find(s => s.name === WM.focusedMonitor?.name) ?? Quickshell.screens[0]
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "quickshell:cakeosUpdater"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: Updates.updaterOpen ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
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
            readonly property var commits: status?.commits ?? []

            function close() {
                Updates.updaterOpen = false;
            }

            // Escape and the "back" button: one step back, or out.
            function back() {
                if (panel.page === "password" && !Updates.applying) {
                    Updates.applyResult = "";
                    panel.step = "overview";
                } else {
                    panel.close();
                }
            }

            function choose(mode) {
                panel.mode = mode;
                panel.step = "password";
            }

            function submit() {
                if (passwordPrompt.text === "") return;
                Updates.apply(passwordPrompt.text, panel.mode);
                passwordPrompt.text = "";
            }

            // what the window is named, in its top edge
            readonly property string label: {
                if (panel.page === "password") return `sudo: update-cakeos apply --mode ${panel.mode}`;
                if (panel.page === "progress") {
                    if (Updates.applyResult === "ok") return "update-cakeos: done";
                    if (Updates.applyResult === "failed") return "update-cakeos: failed";
                    return `update-cakeos: apply --mode ${Updates.applyMode}`;
                }
                return "update-cakeos: check";
            }
            // gold: needs you; foam: done; love: failed
            readonly property string variant: {
                if (panel.page === "password") return "gold";
                if (Updates.applyResult === "ok") return "foam";
                if (Updates.applyResult === "failed") return "love";
                return "pine";
            }

            // The stages the apply script walks through, read from its "==> " lines.
            readonly property bool stageSync: Updates.applying || Updates.applyLog.length > 0
            readonly property bool stageBuild: Updates.applyLog.some(l => l.startsWith("==> Rebuilding"))
            readonly property bool stageActivate: Updates.applyResult === "ok" || Updates.applyLog.some(l => l.startsWith("==> Rebuild completed"))
            readonly property int drvTotal: {
                for (const l of Updates.applyLog) {
                    const m = /these (\d+) derivations? will be built/.exec(l);
                    if (m) return parseInt(m[1]);
                }
                return 0;
            }
            readonly property int drvDone: drvTotal > 0 ? Updates.applyLog.filter(l => l.startsWith("building '")).length : 0

            H.Veil {
                anchors.fill: parent
                heavy: true
                visible: Updates.updaterOpen
                // a click outside the window is a click on "later"; mid-update it hides
                onClicked: panel.close()
            }

            FocusScope {
                anchors.fill: parent
                focus: true
                Keys.onEscapePressed: panel.back()

                H.PixelTransition {
                    id: transition
                    shown: Updates.updaterOpen
                    anchors.centerIn: parent
                    width: win.width
                    height: win.height
                    onClosed: root.lingering = false

                    H.HWindow {
                        id: win
                        width: 620
                        height: implicitHeight
                        label: panel.label
                        variant: panel.variant
                        hints: panel.page === "password" ? [{ key: "enter", text: "run" }, { key: "esc", text: "back" }]
                            : panel.page === "progress" ? [{ key: "esc", text: Updates.applying ? "hide" : "close" }]
                            : [{ key: "esc", text: "close" }]

                        // ---- what's new -------------------------------------------------
                        H.PromptLine {
                            visible: panel.page === "overview"
                            Layout.fillWidth: true
                            path: "/etc/cakeos"
                            command: "update-cakeos"
                            args: "check"
                        }
                        H.HText {
                            visible: panel.page === "overview"
                            Layout.fillWidth: true
                            Layout.topMargin: 12
                            wrapMode: Text.WordWrap
                            textFormat: Text.StyledText
                            ink: panel.status?.error ? "love" : "text"
                            text: {
                                const s = panel.status;
                                const gold = t => `<font color="${H.HTokens.gold}">${root.esc(t)}</font>`;
                                const foam = t => `<font color="${H.HTokens.foam}">${root.esc(t)}</font>`;
                                if (Updates.checking && !s) return "Checking for updates…";
                                if (!s) return "";
                                if (s.error) return `Couldn't check for updates: ${root.esc(s.error)}`;
                                if (s.behind === 0) return `You're on ${gold(String(s.installed ?? "").slice(0, 7))}. ${foam(s.branch)} has nothing newer.`;
                                if (s.behind === null) return `Installed version unknown. ${foam(s.branch)} is at ${gold(String(s.latest ?? "").slice(0, 7))}.`;
                                const n = `${s.behind} ${s.behind === 1 ? "commit" : "commits"}`;
                                return `You're on ${gold(String(s.installed ?? "").slice(0, 7))}. ${foam(s.branch)} is ${n} ahead.`;
                            }
                        }
                        Item {
                            id: changes
                            visible: panel.page === "overview"
                            Layout.fillWidth: true
                            Layout.topMargin: 12
                            Layout.preferredHeight: Math.min(changeList.implicitHeight, 200)
                            clip: true

                            Flickable {
                                anchors.fill: parent
                                contentWidth: width
                                contentHeight: changeList.implicitHeight
                                boundsBehavior: Flickable.StopAtBounds
                                flickableDirection: Flickable.VerticalFlick

                                H.CodeBlock {
                                    id: changeList
                                    width: changes.width
                                    lines: panel.commits.length > 0
                                        ? panel.commits.map(c => `${String(c.sha).slice(0, 7)} ${c.subject} · ${root.ago(c.date)}`)
                                        : [Updates.checking ? "# checking…" : "# nothing new upstream"]
                                }
                            }
                        }
                        H.HRule {
                            visible: panel.page === "overview"
                            Layout.fillWidth: true
                        }
                        H.HText {
                            visible: panel.page === "overview"
                            Layout.fillWidth: true
                            Layout.topMargin: 6
                            wrapMode: Text.WordWrap
                            ink: "iris"
                            text: panel.status?.behind === 0
                                ? "Nothing to apply. You can still rebuild the system you are on."
                                : "The machine can take these now, or quietly on the next boot."
                        }
                        Row {
                            visible: panel.page === "overview"
                            Layout.topMargin: 12
                            spacing: 8
                            H.InkButton {
                                text: panel.status?.behind === 0 ? "rebuild on next boot" : "apply on next boot"
                                ink: "c3"
                                enabled: !Updates.checking
                                onClicked: panel.choose("boot")
                            }
                            H.InkButton {
                                text: panel.status?.behind === 0 ? "rebuild now" : "apply now"
                                ink: "c5"
                                enabled: !Updates.checking
                                onClicked: panel.choose("switch")
                            }
                            H.InkButton {
                                text: "check again"
                                flat: true
                                enabled: !Updates.checking
                                onClicked: Updates.refresh()
                            }
                            H.InkButton {
                                text: "later"
                                ink: "c1"
                                onClicked: panel.close()
                            }
                        }

                        // ---- the password -----------------------------------------------
                        H.PromptLine {
                            visible: panel.page === "password"
                            Layout.fillWidth: true
                            path: "/etc/cakeos"
                            command: "sudo update-cakeos"
                            args: `apply --mode ${panel.mode}`
                        }
                        H.HText {
                            visible: panel.page === "password"
                            Layout.fillWidth: true
                            Layout.topMargin: 12
                            wrapMode: Text.WordWrap
                            text: panel.mode === "boot"
                                ? "CakeOS will download the update and build it. It takes over at the next boot."
                                : "CakeOS will download the update, build it and switch to it right away."
                        }
                        H.PromptLine {
                            id: passwordPrompt
                            visible: panel.page === "password"
                            Layout.fillWidth: true
                            Layout.topMargin: 12
                            path: "password"
                            input: true
                            onAccepted: panel.submit()
                            // masked, with a character the bitmap face has
                            Component.onCompleted: {
                                passwordPrompt.field.echoMode = TextInput.Password;
                                passwordPrompt.field.passwordCharacter = "*";
                            }
                            onVisibleChanged: if (visible) Qt.callLater(() => passwordPrompt.forceInputFocus())
                        }
                        H.HText {
                            visible: Updates.applyResult === "wrongPassword"
                            Layout.fillWidth: true
                            Layout.topMargin: 6
                            ink: "love"
                            text: "sudo: wrong password, try again"
                        }
                        Row {
                            visible: panel.page === "password"
                            Layout.topMargin: 12
                            spacing: 8
                            H.InkButton {
                                text: "run"
                                ink: "c3"
                                enabled: passwordPrompt.text !== ""
                                onClicked: panel.submit()
                            }
                            H.InkButton {
                                text: "back"
                                ink: "c1"
                                onClicked: panel.back()
                            }
                        }

                        // ---- progress and log -------------------------------------------
                        Row {
                            visible: panel.page === "progress"
                            spacing: 8
                            H.Chip { text: "sync"; on: panel.stageSync }
                            H.Chip { text: "build"; on: panel.stageBuild }
                            H.Chip { text: "activate"; on: panel.stageActivate }
                        }
                        H.Progress {
                            visible: panel.page === "progress" && Updates.applyResult !== "failed"
                            Layout.fillWidth: true
                            Layout.topMargin: 10
                            label: Updates.applyStatus
                            count: Updates.applyResult === "ok" ? "done" : panel.drvTotal > 0 ? `${panel.drvDone} / ${panel.drvTotal} drvs` : ""
                            value: Updates.applyResult === "ok" ? 1 : panel.drvTotal > 0 ? panel.drvDone / panel.drvTotal : 0
                        }
                        H.HText {
                            visible: panel.page === "progress" && Updates.applyResult === "failed"
                            Layout.fillWidth: true
                            Layout.topMargin: 10
                            wrapMode: Text.WordWrap
                            ink: "love"
                            text: Updates.applyStatus
                        }
                        Rectangle {
                            visible: panel.page === "progress"
                            Layout.fillWidth: true
                            Layout.topMargin: 10
                            Layout.preferredHeight: 220
                            color: H.HTokens.base
                            clip: true

                            ListView {
                                id: log
                                anchors.fill: parent
                                anchors.margins: 8
                                spacing: 2
                                boundsBehavior: Flickable.StopAtBounds
                                model: Updates.applyLog
                                onCountChanged: positionViewAtEnd()
                                delegate: H.HText {
                                    required property string modelData
                                    width: log.width
                                    wrapMode: Text.WrapAnywhere
                                    role: "code"
                                    ink: modelData.startsWith("==> ") ? "foam" : modelData.startsWith("!!! ") ? "love" : "inkMuted"
                                    text: modelData
                                }
                            }
                        }
                        H.HText {
                            visible: Updates.applying
                            Layout.fillWidth: true
                            Layout.topMargin: 10
                            wrapMode: Text.WordWrap
                            role: "caption"
                            ink: "inkMuted"
                            text: "Hiding this window does not stop the update."
                        }
                        Row {
                            visible: panel.page === "progress"
                            Layout.topMargin: 10
                            spacing: 8
                            H.InkButton {
                                visible: Updates.applyResult === "ok" && Updates.applyMode === "boot"
                                text: "reboot now"
                                ink: "c3"
                                onClicked: Session.reboot()
                            }
                            H.InkButton {
                                visible: Updates.applyResult === "failed"
                                text: "try again"
                                ink: "c3"
                                onClicked: {
                                    Updates.applyResult = "";
                                    panel.step = "overview";
                                }
                            }
                            H.InkButton {
                                text: Updates.applying ? "hide" : "close"
                                ink: "c1"
                                onClicked: panel.close()
                            }
                        }
                    }
                }
            }
        }
    }
}
