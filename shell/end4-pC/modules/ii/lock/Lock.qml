pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.modules.common
import qs.modules.common.functions
import qs.modules.common.panels.lock
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland

LockScreen {
    id: root

    property var savedWorkspaces: ({})

    Timer {
        id: restoreTimer
        interval: 150
        repeat: false
        onTriggered: {
            var batch = ""
            for (var j = 0; j < Quickshell.screens.length; ++j) {
                var monName = Quickshell.screens[j].name
                var wsId = root.savedWorkspaces[monName]
                if (wsId !== undefined) {
                    batch += `hyprctl dispatch 'hl.dsp.focus({monitor="${monName}"})'; hyprctl dispatch 'hl.dsp.focus({workspace=${wsId}})';`
                }
            }
            if (batch.length > 0) {
                Quickshell.execDetached(["bash", "-c", batch])
            }
        }
    }

    lockSurface: LockSurface {
        context: root.context
    }

    Connections {
        target: GlobalStates
        function onScreenLockedChanged() {
            if (GlobalStates.screenLocked) {
                if (WM.niriLike) {
                    return;
                }

                var next = {}
                var batch = "keyword animation workspaces,1,7,menu_decel,slidevert; "
                for (var i = 0; i < Quickshell.screens.length; ++i) {
                    var mon = Quickshell.screens[i].name
                    var mData = HyprlandData.monitors.find(m => m.name === mon)
                    if (mData?.activeWorkspace == undefined) {
                        return;
                    }
                    var ws = (mData?.activeWorkspace?.id ?? 1)
                    next[mon] = ws
                    batch += `hyprctl dispatch 'hl.dsp.focus({monitor="${mon}"})'; hyprctl dispatch 'hl.dsp.focus({workspace=${2147483647 - ws}})';`
                }
                root.savedWorkspaces = next
                Quickshell.execDetached(["bash", "-c", batch])
            } else {
                if (!WM.niriLike) {
                    restoreTimer.start()
                }
            }
        }
    }
}
