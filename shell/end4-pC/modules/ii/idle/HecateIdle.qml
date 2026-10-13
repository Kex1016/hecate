import QtQuick
import Quickshell
import Quickshell.Wayland
import qs
import qs.modules.common

/**
 * hecate: lock, screens off and sleep after the idle times in Config.options.hyprland.idle
 * (seconds, 0 = never), the same settings Hyprland's hypridle gets. ext-idle-notify, so idle
 * inhibitors (the bar's keep-awake toggle, video players, fullscreen games) hold all three off.
 */
Scope {
    id: root
    readonly property var idle: Config.options.hyprland.idle

    function lock() {
        if (!GlobalStates.screenLocked) GlobalStates.screenLocked = true;
    }

    // An IdleMonitor ignores later timeout changes, so each time gets a fresh one.
    component IdleAction: Variants {
        id: action
        required property int seconds
        signal idled()
        model: action.seconds > 0 ? [action.seconds] : []
        IdleMonitor {
            required property int modelData
            timeout: modelData
            respectInhibitors: true
            onIsIdleChanged: if (isIdle) action.idled()
        }
    }

    IdleAction {
        seconds: root.idle.lock
        onIdled: root.lock()
    }

    IdleAction {
        seconds: root.idle.screenOff
        // Any input turns them back on (hecate does that itself).
        onIdled: Quickshell.execDetached(["hecate", "msg", "action", "power_off_outputs"])
    }

    IdleAction {
        seconds: root.idle.suspend
        onIdled: {
            // Wake up locked, unless locking is turned off.
            if (root.idle.lock > 0) root.lock();
            Quickshell.execDetached(["systemctl", "suspend"]);
        }
    }
}
