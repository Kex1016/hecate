pragma Singleton
pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common.functions

Singleton {
    id: root
    property string filePath: Directories.shellConfigPath
    property alias options: configOptionsJsonAdapter
    property bool ready: false
    property int readWriteDelay: 50 // milliseconds
    property bool blockWrites: false

    function setNestedValue(nestedKey, value) {
        let keys = nestedKey.split(".");
        let obj = root.options;
        let parents = [obj];

        // Traverse and collect parent objects
        for (let i = 0; i < keys.length - 1; ++i) {
            if (!obj[keys[i]] || typeof obj[keys[i]] !== "object") {
                obj[keys[i]] = {};
            }
            obj = obj[keys[i]];
            parents.push(obj);
        }

        // Convert value to correct type using JSON.parse when safe
        let convertedValue = value;
        if (typeof value === "string") {
            let trimmed = value.trim();
            if (trimmed === "true" || trimmed === "false" || !isNaN(Number(trimmed))) {
                try {
                    convertedValue = JSON.parse(trimmed);
                } catch (e) {
                    convertedValue = value;
                }
            }
        }

        obj[keys[keys.length - 1]] = convertedValue;
    }

    Timer {
        id: fileReloadTimer
        interval: root.readWriteDelay
        repeat: false
        onTriggered: {
            configFileView.reload()
        }
    }

    Timer {
        id: fileWriteTimer
        interval: root.readWriteDelay
        repeat: false
        onTriggered: {
            configFileView.writeAdapter()
        }
    }

    FileView {
        id: configFileView
        path: root.filePath
        watchChanges: true
        blockWrites: root.blockWrites
        onFileChanged: fileReloadTimer.restart()
        onAdapterUpdated: fileWriteTimer.restart()
        onLoaded: root.ready = true
        onLoadFailed: error => {
            // First start: write the defaults out. Nothing loads after that (a file
            // that did not exist is not watched), so the defaults are what is ready.
            if (error == FileViewError.FileNotFound) {
                writeAdapter();
                root.ready = true;
            }
        }

        JsonAdapter {
            id: configOptionsJsonAdapter

            // The keys that stay are the ones on the settings pages: general, bar, desktop,
            // panels, calendars, services and login screen. There is no theme page: haiiro is
            // the theme, so nothing here sets a colour, a font, a radius, a transparency, a bar
            // position or a bar style. Sizes are tokens too (Appearance.haiiro.size).
            //
            // Keys under a "Legacy" comment are not settings any more. A module written for
            // the old shell still reads each of them; it goes when that module is ported or
            // deleted, so new code must not read them.

            // ── general ──────────────────────────────────────────────────────────────────

            property JsonObject apps: JsonObject {
                property string bluetooth: "kcmshell6 kcm_bluetooth"
                property string changePassword: "kitty -1 --hold=yes fish -i -c 'passwd'"
                property string network: "kcmshell6 kcm_networkmanagement"
                property string networkEthernet: "kcmshell6 kcm_networkmanagement"
                property string taskManager: "plasma-systemmonitor --page-name Processes"
                // hecate: run as root (after a password prompt) by the session menu's Boot Windows.
                property string bootWindows: "~/.local/bin/boot-windows"
                property string terminal: "kitty -1" // This is only for shell actions
                property string volumeMixer: `~/.config/hypr/hyprland/scripts/launch_first_available.sh "pavucontrol-qt" "pavucontrol"`
            }

            property JsonObject audio: JsonObject {
                // Values in %
                property JsonObject protection: JsonObject {
                    // Prevent sudden bangs
                    property bool enable: false
                    property real maxAllowedIncrease: 10
                    property real maxAllowed: 99
                }
            }

            property JsonObject battery: JsonObject {
                property int low: 20
                property int critical: 5
                property int full: 101
                property bool automaticSuspend: true
                property int suspend: 3
            }

            property JsonObject light: JsonObject {
                property JsonObject night: JsonObject {
                    property bool automatic: true
                    property string from: "19:00" // Format: "HH:mm", 24-hour time
                    property string to: "06:30"   // Format: "HH:mm", 24-hour time
                    property int colorTemperature: 5000
                }
                // Legacy (quick panel, Brightness)
                property JsonObject antiFlashbang: JsonObject {
                    property bool enable: false
                }
            }

            // Reduced motion: every transition is a cut and the background is one still frame.
            // Game Mode does the same while it is on (Appearance.gameMode).
            property JsonObject motion: JsonObject {
                property bool reduced: false
            }

            property JsonObject profile: JsonObject {
                property string displayName: ""
            }

            property JsonObject resources: JsonObject {
                property int updateInterval: 3000
                property int historyLength: 60
            }

            property JsonObject sounds: JsonObject {
                property bool battery: false
                property bool pomodoro: false
                property string theme: "freedesktop"
            }

            property JsonObject time: JsonObject {
                // The clock is 24h ("hh:mm") or 12h ("h:mm ap").
                // https://doc.qt.io/qt-6/qtime.html#toString
                property string format: "hh:mm"
                property string shortDateFormat: "dd/MM"
                property string dateWithYearFormat: "dd/MM/yyyy"
                property string dateFormat: "ddd, dd/MM"
                property JsonObject pomodoro: JsonObject {
                    property int breakTime: 300
                    property int cyclesBeforeLongBreak: 4
                    property int focus: 1500
                    property int longBreak: 900
                }
                // Seconds in the bar.
                property bool secondPrecision: false
            }

            property JsonObject updates: JsonObject {
                property bool enableCheck: true
                property int checkInterval: 120 // minutes
                property int adviseUpdateThreshold: 75 // packages
                property int stronglyAdviseUpdateThreshold: 200 // packages
            }

            // The names are Hyprland's; on hecate these are the shell's own. Compositor
            // settings (windows, focus, effects, displays, mouse) live in
            // ~/.config/hecate/shell.toml.
            property JsonObject hyprland: JsonObject {
                property JsonObject autostartApps: JsonObject {
                    property bool enable: false
                    property list<var> apps: []
                }
                // Seconds.
                property JsonObject idle: JsonObject {
                    property int lock: 300
                    property int screenOff: 600
                    property int suspend: 10800
                }
            }

            // ── bar ──────────────────────────────────────────────────────────────────────
            // One bar, always at the top, with fixed segments.

            property JsonObject bar: JsonObject {
                // Mod+J hides and shows it.
                property JsonObject autoHide: JsonObject {
                    property bool enable: false
                    property JsonObject showWhenPressingSuper: JsonObject {
                        property bool enable: true
                    }
                }
                property JsonObject workspaces: JsonObject {
                    property int shown: 10
                    // Holding Super names each workspace's top app after this long (milliseconds).
                    property int showNumberDelay: 300
                }
                property JsonObject resources: JsonObject {
                    property int memoryWarningThreshold: 95
                    property int swapWarningThreshold: 85
                    property int cpuWarningThreshold: 90
                }
                // The weather service's settings (the clock's hover, the weather widget).
                property JsonObject weather: JsonObject {
                    property bool enable: false
                    property bool enableGPS: true // gps based location
                    property string city: "" // When 'enableGPS' is false
                    property bool useUSCS: false // Instead of metric (SI) units
                    property int fetchInterval: 10 // minutes
                }
                // The Claude usage service's settings.
                property JsonObject aiUsage: JsonObject {
                    property int tokenLimit: 1000000
                    property int updateInterval: 60
                }

            }

            property JsonObject tray: JsonObject {
                property bool monochromeIcons: true
                property bool showItemId: false
                property bool invertPinnedItems: true // Makes the below a whitelist for the tray and blacklist for the pinned area
                property list<var> pinnedItems: [ "Fcitx" ]
                property bool filterPassive: true
            }

            // ── desktop ──────────────────────────────────────────────────────────────────

            property JsonObject background: JsonObject {
                // "blobs": the live simulation, drawn by the compositor | "wallpaper": your own
                // images, which also brings the wallpaper picker back.
                property string mode: "blobs"
                // Wallpaper only: press the picture into two tones of the palette.
                property bool dither: true
                // Wallpaper only: a picture per monitor (monitorWallpapers).
                property bool perMonitor: false
                property string wallpaperPath: ""
                // Monitors with a wallpaper of their own: [{ monitor: "DP-1", path: "/..." }].
                // The rest show wallpaperPath.
                property list<var> monitorWallpapers: []
                // Wallpaper only: "folder" | "icloudAlbum" (album names it).
                property string source: "folder"
                property string album: ""
                // The desktop clock.
                property JsonObject clock: JsonObject {
                    // "display": Jacquard 24 at 150px | "vga": the bitmap face at 32px | "none"
                    property string style: "display"
                    // The rose sentence under the clock (weather, battery, what's next).
                    property bool narration: true
                    // "bottomLeft" | "bottomRight" | "centre"
                    property string corner: "bottomLeft"
                }

                // Legacy (background, lock, wallpaper picker, old desktop widgets, shell.qml)
                property string lockWall: ""
                property bool widgetsLocked: false
                property bool showGrid: true
            }

            property JsonObject lock: JsonObject {
                property bool launchOnStartup: false
                // The media strip.
                property bool showMedia: true
                // "none" | "count" | "all"
                property string notifications: "count"
                property JsonObject security: JsonObject {
                    property bool unlockKeyring: true
                    property bool requirePasswordToPower: false
                }
                // Legacy (lock, background)
                property bool useHyprlock: false
            }

            // Toasts stack top right; critical ones stay until dismissed.
            property JsonObject notifications: JsonObject {
                // Milliseconds.
                property int timeout: 5000
                property bool silentInFullscreen: true
                property bool sounds: false
            }

            property JsonObject osd: JsonObject {
                // Milliseconds.
                property int timeout: 1500
            }

            // ── panels ───────────────────────────────────────────────────────────────────

            property JsonObject launcher: JsonObject {
                // Your own names for apps: { "files": "yazi", "web": "zen" }. An exact alias
                // wins over every other match.
                property var aliases: ({})
            }

            property JsonObject search: JsonObject {
                property int nonAppResultDelay: 30 // This prevents lagging when typing
                property string engineBaseUrl: "https://duckduckgo.com/?q="
                property list<string> excludedSites: ["quora.com", "facebook.com"]
                property list<var> clipboardPins: []
                property bool sloppy: false // Uses levenshtein distance based scoring instead of fuzzy sort. Very weird.
                property JsonObject imageSearch: JsonObject {
                    property string imageSearchEngineBaseUrl: "https://lens.google.com/uploadbyurl?url="
                    property bool useCircleSelection: false
                }
            }

            property JsonObject language: JsonObject {
                // The translate tools (tr in the launcher, Mod+Shift+T).
                property JsonObject translator: JsonObject {
                    property string targetLanguage: "auto" // Run `trans -list-all` for available languages
                }
                // Legacy (translations). The shell is English only.
                property string ui: "auto"
            }

            property JsonObject media: JsonObject {
                // Attempt to remove dupes (the aggregator playerctl one and browsers' native ones when there's plasma browser integration)
                property bool filterDuplicatePlayers: true
            }

            property JsonObject regionSelector: JsonObject {
                property JsonObject targetRegions: JsonObject {
                    property bool windows: true
                    property bool layers: false
                    property bool content: true
                    property bool showLabel: false
                    property real opacity: 0.3
                    property real contentRegionOpacity: 0.8
                    property int selectionPadding: 5
                }
                property JsonObject rect: JsonObject {
                    property bool showAimLines: true
                }
                property JsonObject circle: JsonObject {
                    property int strokeWidth: 6
                    property int padding: 10
                }
                property JsonObject annotation: JsonObject {
                    property bool useSatty: false
                }
            }

            property JsonObject screenRecord: JsonObject {
                property string savePath: Directories.videos.replace("file://","") // strip "file://"
                property bool systemAudio: false
                property bool microphone: false
            }

            property JsonObject screenSnip: JsonObject {
                property string savePath: "" // only copy to clipboard when empty
            }

            // ── calendars ────────────────────────────────────────────────────────────────
            // Accounts are in ~/.config/hecate/calendars.toml (Directories.calendarsPath); their
            // secrets are in the keyring.

            property JsonObject calendars: JsonObject {
                property int syncInterval: 15 // minutes
                // Where events added from the dashboard go: "local", or an account's calendar
                // such as "icloud: home".
                property string newEventsTo: "local"
            }

            // ── services ─────────────────────────────────────────────────────────────────

            // Claude is opt-in: nothing is installed or sent until this is on.
            property JsonObject claude: JsonObject {
                property bool enable: false
                // Claude Code's hooks report to the shell (the sessions panel, its toasts and
                // the gold window border). notifications: also put each permission request in
                // the notification list (the toast is the sessions panel's own).
                property JsonObject bridge: JsonObject {
                    property bool enable: true
                    property bool notifications: true
                }
            }

            // AirPods (librepods), iCloud Photos and Drive, and Cider, each off until switched
            // on (Photos needs wallpaper mode and an album). The Apple ID is the
            // first iCloud account in calendars.toml unless appleId names one.
            property JsonObject apple: JsonObject {
                property string appleId: ""
                property JsonObject airpods: JsonObject {
                    property bool enable: false
                    // Taking a bud out pauses what plays.
                    property bool pauseOnRemove: true
                }
                property JsonObject photos: JsonObject {
                    property int rotate: 60 // minutes
                    property int syncHours: 6
                }
                property JsonObject drive: JsonObject {
                    property bool enable: false
                    property string remote: "icloud"
                    property string path: "~/iCloud"
                }
                property JsonObject cider: JsonObject {
                    property bool enable: false
                    property string url: "http://localhost:10767"
                    // "" reads it from the keyring (application hecate service cider)
                    property string token: ""
                }
            }

            property JsonObject conflictKiller: JsonObject {
                property bool autoKillNotificationDaemons: false
                property bool autoKillTrays: false
            }

            property JsonObject hacks: JsonObject {
                property int arbitraryRaceConditionDelay: 20 // milliseconds
            }

            // The repos desktop widget: git checkouts under path, fetched in the background.
            property JsonObject repos: JsonObject {
                property string path: "~/src"
                property bool fetch: true
            }

            // ── login screen ─────────────────────────────────────────────────────────────
            // hecate-greetd always shows the blobs. Your keyboard and monitor layout still come
            // along (`hecate-greetd sync`).

            property JsonObject greeter: JsonObject {
                property bool share: true
                // Hand over your monitor layout and keyboard/mouse settings.
                property bool displays: true
                property bool input: true
                property bool showStatusText: true
                property string statusText: "" // "" = "Locked", like the lock screen
            }

            // ── Legacy sections ──────────────────────────────────────────────────────────
            // Whole sections that belong to modules being deleted or rebuilt. None of them is
            // a setting any more.

            // shell.qml loads the panel family by this name.

            // The old settings window.
            property JsonObject settings: JsonObject {
                property string style: "default" // default - minimal - dashboard
                property list<string> collapsedSections: []
            }

            // The old wallpaper picker and the online search.
            property JsonObject wallpaperSelector: JsonObject {
                property bool closeAfterSelection: true
                property string sortMode: "time"
            }

            // The welcome and kill dialogs' own title bar.
            property JsonObject windows: JsonObject {
                property bool showTitlebar: true // Client-side decoration for shell apps
                property bool centerTitle: true
            }

        }
    }
}
