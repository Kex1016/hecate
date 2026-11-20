import QtQuick
import qs
import qs.modules.common
import qs.services
import qs.modules.ii.settings

SettingsPage {
    id: page

    readonly property bool wallpaper: Config.options.background.mode === "wallpaper"

    SetColumn {
        SetSection { text: "background" }
        SetChoice {
            name: "background"
            description: "Blobs: the live simulation, the default. Wallpaper: your own images, and a wallpapers tab appears here."
            key: "background.mode"
            values: ["blobs", "wallpaper"]
        }
        SetToggle {
            name: "dither to haiiro"
            description: "Wallpaper only. Two palette tones, Bayer, 2× pixels."
            key: "background.dither"
            enabled: page.wallpaper
        }
        SetToggle {
            name: "per-monitor images"
            description: "Wallpaper only."
            key: "background.perMonitor"
            enabled: page.wallpaper
        }
        SetAction {
            name: "pick wallpapers"
            description: "Wallpaper only. Also Mod+Ctrl+T."
            buttons: [{ text: "open picker", ink: "c1" }]
            enabled: page.wallpaper
            onPressed: {
                GlobalStates.settingsOpen = false;
                GlobalStates.wallpaperSelectorOpen = true;
            }
        }
        SetSection { text: "desktop clock" }
        SetChoice {
            name: "style"
            description: "display: Jacquard 24 at 150px. vga: the bitmap face at 32px."
            key: "background.clock.style"
            values: ["display", "vga", "none"]
        }
        SetToggle {
            name: "narration line"
            description: "The rose sentence under the clock (weather, battery, what's next)."
            key: "background.clock.narration"
            enabled: Config.options.background.clock.style !== "none"
        }
        SetChoice {
            name: "corner"
            key: "background.clock.corner"
            values: ["bottomLeft", "bottomRight", "centre"]
            labels: ["bottom left", "bottom right", "centre"]
            enabled: Config.options.background.clock.style !== "none"
        }
    }
    SetColumn {
        SetSection { text: "lock screen" }
        SetToggle {
            name: "lock on startup"
            key: "lock.launchOnStartup"
        }
        SetToggle {
            name: "media controls"
            key: "lock.showMedia"
        }
        SetChoice {
            name: "show notifications"
            key: "lock.notifications"
            values: ["none", "count", "all"]
        }
        SetSection { text: "notifications" }
        SetNumber {
            name: "toast timeout"
            key: "notifications.timeout"
            from: 1
            to: 30
            divisor: 1000
            suffix: "s"
        }
        SetToggle {
            name: "silent in fullscreen"
            key: "notifications.silentInFullscreen"
        }
        SetToggle {
            name: "sounds"
            key: "notifications.sounds"
        }
        SetSection { text: "time" }
        SetChoice {
            name: "clock"
            key: "time.format"
            values: ["hh:mm", "h:mm ap"]
            labels: ["24h", "12h"]
        }
        SetToggle {
            name: "seconds in the bar"
            key: "time.secondPrecision"
        }
        SetSection { text: "desktop widgets" }
        SetText {
            name: "race"
            description: "The character sheet's first line of flavour."
            value: DesktopWidgets.race
            onEdited: v => DesktopWidgets.setProfile(String(v).trim() || "human", DesktopWidgets.klass)
        }
        SetText {
            name: "class"
            description: "And its second."
            value: DesktopWidgets.klass
            onEdited: v => DesktopWidgets.setProfile(DesktopWidgets.race, String(v).trim() || "nix tinkerer")
        }
        SetText {
            name: "repos folder"
            description: "The repos widget lists the git checkouts in here."
            key: "repos.path"
            placeholder: "~/src"
        }
        SetToggle {
            name: "fetch repos"
            description: "git fetch them in the background, so ahead/behind is current."
            key: "repos.fetch"
        }
    }
}
