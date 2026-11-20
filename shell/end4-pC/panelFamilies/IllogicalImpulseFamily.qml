import QtQuick
import Quickshell

import qs
import qs.modules.common
import qs.modules.ii.background
import qs.modules.ii.bar
import qs.modules.ii.equalizer
import qs.modules.ii.lock
import qs.modules.ii.mediaControls
import qs.modules.ii.notificationPopup
import qs.modules.ii.onScreenDisplay
import qs.modules.ii.overview
import qs.modules.ii.polkit
import qs.modules.ii.settings
import qs.modules.ii.regionSelector
import qs.modules.ii.screenTranslator
import qs.modules.ii.sessionScreen
import qs.modules.ii.sidebarRight
import qs.modules.ii.wallpaperSelector
import qs.modules.ii.desktopMenu
import qs.modules.ii.idle
import qs.modules.ii.sharePicker
import qs.modules.ii.altTab
import qs.modules.ii.cakeosUpdater
import qs.modules.ii.claude
import qs.services

Scope {
    // Loaded in the background in this order (PanelLoader): what's on screen first, then the
    // lock screen and popups, then what only opens on demand.

    // On screen
    PanelLoader { id: background; component: Background {} }
    PanelLoader { id: bar; component: Bar {} }

    // hecate's login splash (a picture of this desktop) covers the screen until the wallpaper
    // and bar are up; the delay lets their first frames land before it fades.
    Timer {
        interval: 300
        running: WM.compositor === "hecate" && background.item !== null
            && bar.item !== null && !GlobalStates.startupLockPending
        onTriggered: Quickshell.execDetached(["hecate", "msg", "action", "splash-done"])
    }

    // Lock screen, notifications, OSD
    PanelLoader { component: Lock {} }
    PanelLoader { component: NotificationPopup {} }
    PanelLoader { component: OnScreenDisplay {} }
    PanelLoader { extraCondition: WM.compositor === "hecate"; component: HecateIdle {} }

    // On demand
    PanelLoader { component: Polkit {} }
    PanelLoader { component: SessionScreen {} }
    PanelLoader { extraCondition: WM.compositor === "hecate"; component: AltTab {} }
    PanelLoader { extraCondition: WM.compositor === "hecate"; component: SharePicker {} }
    PanelLoader { component: SidebarRight {} }
    PanelLoader { component: Overview {} }
    // Claude is opt-in (claude.enable): Mod+/, Mod+Shift+/, the launcher's `?` and the
    // calendar and music cards exist only once it is on.
    PanelLoader { extraCondition: Config.options.claude.enable; component: Sessions {} }
    PanelLoader { extraCondition: Config.options.claude.enable; component: AskPanel {} }
    PanelLoader { extraCondition: Config.options.claude.enable; component: AppleCardsPanel {} }
    PanelLoader { component: MediaControls {} }
    PanelLoader { component: EqualizerPopup {} }
    PanelLoader { component: RegionSelector {} }
    PanelLoader { component: ScreenTranslator {} }
    PanelLoader { component: DesktopMenu {} }
    PanelLoader { component: WallpaperSelector {} }
    PanelLoader { component: Settings {} }
    PanelLoader { extraCondition: Updates.cakeos; component: CakeosUpdater {} }
}
