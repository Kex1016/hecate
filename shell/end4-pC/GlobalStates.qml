import qs.modules.common
import qs.services
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
pragma Singleton
pragma ComponentBehavior: Bound

Singleton {
    id: root
    property bool barOpen: true
    property bool equalizerOpen: false
    property bool sidebarRightOpen: false
    property bool mediaControlsOpen: false
    // the bar's media segment is showing its hover, with the visualiser strip
    property bool barMediaHover: false
    property bool settingsOpen: false
    property bool osdVolumeOpen: false
    property bool overviewOpen: false
    property bool regionSelectorOpen: false
    property bool screenLocked: false
    property bool screenLockContainsCharacters: false
    property bool screenUnlockFailed: false
    // Replaces "Locked" under the lock screen clock (hecate-greetd sets its own).
    property string lockStatusText: ""
    // Running as hecate-greetd, the login screen: the lock screen with nothing behind it.
    property bool greeterMode: false
    property bool screenTranslatorOpen: false
    property bool sessionOpen: false
    property bool superDown: false
    property bool wallpaperSelectorOpen: false
    // Closing the selector without picking ends a lock-screen or per-monitor pick.
    onWallpaperSelectorOpenChanged: if (!wallpaperSelectorOpen) wallpaperSelectorTarget = "wallpaper"
    property var settingsTarget: null

    function openSettingsAt(pageId, label, section, subsection) {
        root.settingsOpen = true;
        Qt.callLater(() => {
            root.settingsTarget = { page: pageId, label: label ?? "", section: section ?? "", subsection: subsection ?? "" };
        });
    }
    property list<real> visualizerPoints: []
    property bool desktopWidgetKeyboardFocus: false
    property string wallpaperSelectorTarget: "wallpaper"
    property string osdIndicatorType: "volume"
    property int dashboardPage: 0
    property string dashboardGroup: ""
    // Hides the wallpaper and the bar at startup while the lock screen may still be coming up
    // (lock.launchOnStartup). Cleared as soon as the config and persistent state say it isn't,
    // otherwise once the lock is secure (LockScreen), or after the timer at the latest. This
    // used to wait on the lock panel noticing the config load, which never happened when the
    // panel loaded after it, so every start showed a bare screen until the timer.
    property bool startupLockPending: true

    function settleStartupLock() {
        if (!root.startupLockPending || !Config.ready || !Persistent.ready) return;
        if (!(Config.options.lock.launchOnStartup && Persistent.isNewHyprlandInstance))
            root.startupLockPending = false;
    }
    Component.onCompleted: root.settleStartupLock()
    Connections {
        target: Config
        function onReadyChanged() { root.settleStartupLock(); }
    }
    Connections {
        target: Persistent
        function onReadyChanged() { root.settleStartupLock(); }
    }

    Timer {
        interval: 3000
        running: true
        onTriggered: root.startupLockPending = false
    }

    onSidebarRightOpenChanged: {
        if (GlobalStates.sidebarRightOpen) {
            Notifications.timeoutAll();
            Notifications.markAllRead();
        }
    }

    CompositorGlobalShortcut {
        name: "workspaceNumber"
        description: "Hold to show workspace numbers, release to show icons"
        onPressed: { root.superDown = true }
        onReleased: { root.superDown = false }
    }
}
