pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Services.UPower
import qs
import qs.services
import qs.modules.common
import qs.modules.common.functions
import qs.modules.common.panels.lock
import qs.modules.common.widgets.haiiro as H

// The lock screen (and, in hecate-greetd, the login screen): the clock, one
// login window, and a strip along the bottom. The surface itself is clear: the
// compositor's blobs show through a checker veil, nothing is captured or blurred.
MouseArea {
    id: root
    // A LockContext, or anything with the same interface (hecate-greetd's GreeterContext).
    required property var context
    property bool active: false
    property bool showInputField: active || context.currentText.length > 0
    // Set by hecate-greetd: there is no session yet, so power actions need no password, and
    // `userComponent` (the user and session pickers) takes the username's place.
    readonly property bool greeter: root.context.greeter ?? false
    property Component userComponent: null
    readonly property bool requirePasswordToPower: !root.greeter && Config.options.lock.security.requirePasswordToPower
    // What the prompt asks for. A login can ask for more than a password (a one-time code,
    // a new password); greetd says whether the answer is secret.
    readonly property bool secretPrompt: root.context.secretPrompt ?? true

    readonly property bool failed: GlobalStates.screenUnlockFailed
    readonly property bool powerTarget: root.context.targetAction !== LockContext.ActionEnum.Unlock
    readonly property string promptLabel: (root.context.promptText ?? "") || "password:"
    readonly property string failureLabel: (root.context.failureText ?? "") || "That's not it."
    readonly property string notifyMode: Config.options.lock.notifications
    // When this surface appeared, for "locked 4 minutes ago".
    readonly property date lockedAt: new Date()
    readonly property int lockedMinutes: Math.max(0, Math.floor((DateTime.clock.date - root.lockedAt) / 60000))

    // Force focus on entry
    function forceFieldFocus() {
        passwordBox.forceActiveFocus();
    }
    Connections {
        target: context
        function onShouldReFocus() {
            forceFieldFocus();
        }
    }
    hoverEnabled: true
    acceptedButtons: Qt.LeftButton
    onPressed: mouse => {
        forceFieldFocus();
    }
    onPositionChanged: mouse => {
        forceFieldFocus();
    }

    Component.onCompleted: forceFieldFocus()

    // Key presses
    property bool ctrlHeld: false
    Keys.onPressed: event => {
        root.context.resetClearTimer();
        if (event.key === Qt.Key_Control) {
            root.ctrlHeld = true;
        }
        if (event.key === Qt.Key_Escape) { // Esc to clear
            root.context.currentText = "";
        }
        forceFieldFocus();
    }
    Keys.onReleased: event => {
        if (event.key === Qt.Key_Control) {
            root.ctrlHeld = false;
        }
        forceFieldFocus();
    }

    // The desktop behind, dimmed by the checker veil. It takes no clicks of its own.
    H.Veil {
        anchors.fill: parent
        z: -1
        blocking: false
    }

    // Clock
    Column {
        anchors {
            top: parent.top
            topMargin: Math.round(root.height * 120 / 900)
            horizontalCenter: parent.horizontalCenter
        }
        spacing: H.HTokens.space3

        Item {
            anchors.horizontalCenter: parent.horizontalCenter
            width: clockText.implicitWidth + clockShadow.x
            height: clockText.implicitHeight + clockShadow.y

            // the hard offset under a display title
            H.HText {
                id: clockShadow
                x: Appearance.haiiro.shadow.title.x
                y: Appearance.haiiro.shadow.title.y
                role: "display"
                text: clockText.text
                color: Appearance.haiiro.shadow.title.color
            }
            H.HText {
                id: clockText
                role: "display"
                text: DateTime.time
            }
        }
        H.HText {
            anchors.horizontalCenter: parent.horizontalCenter
            role: "nav"
            text: Qt.locale().toString(DateTime.clock.date, "dddd, d MMMM").toLowerCase()
        }
        H.HText {
            anchors.horizontalCenter: parent.horizontalCenter
            ink: "c4"
            text: GlobalStates.lockStatusText.length > 0 ? GlobalStates.lockStatusText : "The screen is locked. It waits for you."
            visible: !root.greeter || GlobalStates.lockStatusText.length > 0
        }
    }

    // Login window
    H.PixelTransition {
        id: loginTransition
        anchors.horizontalCenter: parent.horizontalCenter
        y: Math.round(root.height * 470 / 900)

        H.HWindow {
            id: loginWindow
            width: 520
            label: "login: " + (SystemInfo.hostname.length > 0 ? SystemInfo.hostname : "cakeos")
            variant: root.powerTarget ? "gold" : "pine"

            // who is locked out; the greeter's picker takes this place
            RowLayout {
                Layout.fillWidth: true
                spacing: H.HTokens.space3
                visible: root.userComponent === null

                H.Dither {
                    tone: "pine"
                    implicitWidth: 48
                    implicitHeight: 48
                    Layout.preferredWidth: 48
                    Layout.preferredHeight: 48
                }
                ColumnLayout {
                    spacing: 0
                    H.HText {
                        role: "valueLg"
                        ink: "c5"
                        text: SystemInfo.username
                    }
                    H.HText {
                        role: "caption"
                        ink: "c1"
                        text: root.lockedMinutes < 1 ? "locked just now"
                            : "locked " + root.lockedMinutes + (root.lockedMinutes === 1 ? " minute ago" : " minutes ago")
                    }
                }
            }
            Loader {
                Layout.fillWidth: true
                active: root.userComponent !== null
                visible: active
                sourceComponent: root.userComponent
            }

            // the prompt line: @ user » password: ********█
            Item {
                id: promptLine
                Layout.fillWidth: true
                implicitHeight: H.HTokens.lineBox(H.HTokens.fontBody)
                readonly property string typed: root.context.currentText
                readonly property string shown: root.secretPrompt ? "*".repeat(typed.length) : typed
                readonly property int gap: H.HTokens.ch()

                Row {
                    id: promptHead
                    spacing: promptLine.gap
                    H.HText { text: "@"; ink: "c2" }
                    H.HText { text: SystemInfo.username; ink: "c6" }
                    H.HText { text: "»"; ink: "c4" }
                    H.HText { text: root.promptLabel; ink: "c3" }
                }
                H.HText {
                    id: typedText
                    x: promptHead.width + promptLine.gap
                    width: Math.max(0, parent.width - x - cursor.width)
                    ink: "c7"
                    text: promptLine.shown
                    elide: Text.ElideLeft
                    font.letterSpacing: root.secretPrompt ? 2 : 0
                }
                Rectangle {
                    id: cursor
                    x: Math.min(promptHead.width + promptLine.gap + typedText.contentWidth, promptLine.width - width)
                    anchors.verticalCenter: parent.verticalCenter
                    width: H.HTokens.ch()
                    height: H.HTokens.fontBody
                    color: H.HTokens.text
                    // a steady block, it never blinks
                    visible: !root.context.unlockInProgress
                }

                // the real field: invisible, it only takes the keys
                TextInput {
                    id: passwordBox
                    width: 1
                    height: 1
                    opacity: 0
                    enabled: !root.context.unlockInProgress
                    echoMode: root.secretPrompt ? TextInput.Password : TextInput.Normal
                    inputMethodHints: root.secretPrompt ? Qt.ImhSensitiveData : Qt.ImhNone
                    onTextChanged: root.context.currentText = text
                    onAccepted: root.context.tryUnlock(root.ctrlHeld)
                    Connections {
                        target: root.context
                        function onCurrentTextChanged() {
                            passwordBox.text = root.context.currentText;
                        }
                    }
                    Keys.onPressed: event => {
                        root.context.resetClearTimer();
                    }
                }
            }

            H.HText {
                Layout.fillWidth: true
                role: "caption"
                ink: root.powerTarget ? "c3" : "c1"
                elide: Text.ElideRight
                text: {
                    if (root.context.unlockInProgress)
                        return "checking…";
                    if (root.context.targetAction === LockContext.ActionEnum.Poweroff)
                        return "enter to shut down";
                    if (root.context.targetAction === LockContext.ActionEnum.Reboot)
                        return "enter to reboot";
                    if (root.ctrlHeld)
                        return "enter to unlock and keep the screen awake";
                    if (root.context.fingerprintsConfigured && root.context.currentText.length === 0)
                        return "touch the fingerprint reader · or type your password";
                    return "enter to " + (root.greeter ? "log in" : "unlock") + " · esc to clear";
                }
            }
        }
    }

    // Wrong password
    H.PixelTransition {
        id: failTransition
        anchors.horizontalCenter: parent.horizontalCenter
        y: loginTransition.y + loginWindow.height + 20
        shown: root.failed

        H.HWindow {
            width: 520
            variant: "love"
            label: "login: " + (SystemInfo.hostname.length > 0 ? SystemInfo.hostname : "cakeos")

            RowLayout {
                Layout.fillWidth: true
                spacing: H.HTokens.ch()
                H.HText { text: "@"; ink: "c2" }
                H.HText { text: SystemInfo.username; ink: "c6" }
                H.HText { text: "»"; ink: "c4" }
                H.HText {
                    ink: "c2"
                    text: root.failureLabel
                }
                H.HText {
                    ink: "c1"
                    text: "Try again."
                    visible: (root.context.failureText ?? "") === ""
                }
                Item { Layout.fillWidth: true }
            }
        }
    }

    // Bottom strip
    RowLayout {
        id: strip
        anchors {
            left: parent.left
            right: parent.right
            bottom: parent.bottom
            leftMargin: 24
            rightMargin: 24
            bottomMargin: 20
        }
        spacing: H.HTokens.space4

        // the media strip
        Loader {
            id: mediaStrip
            active: !root.greeter && Config.options.lock.showMedia && MprisController.activePlayer !== null
            visible: active
            Layout.alignment: Qt.AlignVCenter
            Layout.maximumWidth: Math.round(root.width * 0.5)

            sourceComponent: RowLayout {
                id: media
                readonly property var player: MprisController.activePlayer
                readonly property string title: StringUtils.cleanMusicTitle(player?.trackTitle) || ""
                readonly property string artist: player?.trackArtist || ""
                spacing: H.HTokens.space4

                H.HText {
                    Layout.fillWidth: true
                    Layout.maximumWidth: 420
                    ink: "c7"
                    elide: Text.ElideRight
                    text: "♪ " + media.title + (media.artist.length > 0 ? " — " + media.artist : "")
                }
                H.Transport {
                    small: true
                    playing: MprisController.isPlaying
                    canPrevious: MprisController.canGoPrevious
                    canNext: MprisController.canGoNext
                    onPrevious: MprisController.previous()
                    onPlayPause: MprisController.togglePlaying()
                    onNext: MprisController.next()
                }
            }
        }

        Item { Layout.fillWidth: true }

        // keyboard layout
        H.HText {
            text: HyprlandXkb.currentLayoutCode
            visible: text.length > 0
            Layout.alignment: Qt.AlignVCenter
        }
        // battery
        RowLayout {
            visible: Battery.available
            spacing: H.HTokens.ch()
            Layout.alignment: Qt.AlignVCenter
            H.HText { text: Battery.isCharging ? "chg" : "bat" }
            H.HText {
                text: Math.round(Battery.percentage * 100) + "%"
                ink: Battery.isLow && !Battery.isCharging ? "c2" : "c6"
            }
        }
        // unread notifications
        Rectangle {
            visible: !root.greeter && root.notifyMode !== "none" && Notifications.unread > 0
            Layout.alignment: Qt.AlignVCenter
            implicitWidth: unreadText.implicitWidth + 2 * H.HTokens.ch()
            implicitHeight: unreadText.implicitHeight
            color: H.HTokens.love
            H.HText {
                id: unreadText
                anchors.centerIn: parent
                text: Notifications.unread
                ink: "c8"
            }
        }

        H.InkButton {
            ink: "c1"
            text: "sleep"
            onClicked: Session.suspend()
        }
        PowerButton {
            idleInk: "c3"
            text: "reboot"
            targetAction: LockContext.ActionEnum.Reboot
        }
        PowerButton {
            idleInk: "c2"
            text: "shutdown"
            targetAction: LockContext.ActionEnum.Poweroff
        }
    }

    // What the notifications are, when the setting asks for more than a count.
    Column {
        visible: !root.greeter && root.notifyMode === "all" && Notifications.list.length > 0
        anchors {
            left: parent.left
            leftMargin: 24
            bottom: strip.top
            bottomMargin: H.HTokens.space3
        }
        spacing: 0
        Repeater {
            model: Notifications.list.slice(-3)
            H.HText {
                required property var modelData
                width: Math.min(implicitWidth, root.width * 0.5)
                elide: Text.ElideRight
                role: "caption"
                ink: "c4"
                text: modelData.appName + ": " + modelData.summary
            }
        }
    }

    // A power button that waits for the password first, when the setting says so.
    component PowerButton: H.InkButton {
        id: guarded
        required property var targetAction
        // armed: the next enter does it
        property string idleInk: ""
        ink: root.context.targetAction === guarded.targetAction ? "" : idleInk

        onClicked: {
            if (!root.requirePasswordToPower) {
                root.context.unlocked(guarded.targetAction);
                return;
            }
            if (root.context.targetAction === guarded.targetAction) {
                root.context.resetTargetAction();
            } else {
                root.context.targetAction = guarded.targetAction;
                root.context.shouldReFocus();
            }
        }
    }
}
