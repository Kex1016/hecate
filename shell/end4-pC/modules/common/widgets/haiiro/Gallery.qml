pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro

// Every part of the haiiro kit, laid out like the design system's previews,
// for checking them side by side. Load it in any window:
//   FloatingWindow { implicitWidth: 1500; implicitHeight: 900; color: "#191724"
//                    Gallery { anchors.fill: parent } }
Rectangle {
    id: root

    color: HTokens.base
    implicitWidth: 1500
    implicitHeight: sheet.implicitHeight + 48

    // replay every PixelTransition on the sheet
    property bool shown: true
    property bool slow: false
    // behind the free section: a still of the blobs, as the design system's
    // free previews have. Left empty the cards sit on plain base.
    property url freeBackground: ""
    // scroll position, and where the free section starts
    property alias scrollY: flick.contentY
    readonly property real freeY: sheet.y + freeSheet.y

    // one free-form preview: the part at 56,44 on the background, like the
    // design system's cards
    component FreeCard: Item {
        id: freeCard
        property string name: ""
        default property alias body: freeInner.data
        implicitWidth: 700
        implicitHeight: 200
        clip: true
        Rectangle { anchors.fill: parent; color: HTokens.base }
        Image {
            anchors.fill: parent
            visible: root.freeBackground.toString().length > 0
            source: root.freeBackground
            fillMode: Image.PreserveAspectCrop
            smooth: false
        }
        Item {
            id: freeInner
            x: 56
            y: 44
            width: parent.width - 112
            height: parent.height - 88
        }
        HText {
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: 6
            text: freeCard.name
            role: "badge"
            ink: "inkMuted"
        }
    }

    component Card: ColumnLayout {
        id: card
        property string name: ""
        default property alias body: inner.data
        spacing: 8
        HLabel { text: card.name }
        Item {
            id: inner
            Layout.fillWidth: true
            implicitWidth: childrenRect.width
            implicitHeight: childrenRect.height
        }
    }

    Flickable {
        id: flick
        anchors.fill: parent
        contentWidth: width
        contentHeight: sheet.implicitHeight + 48
        clip: true

        ColumnLayout {
            id: sheet
            x: 24
            y: 24
            width: root.width - 48
            spacing: 32

            RowLayout {
                Layout.fillWidth: true
                spacing: 32

                ColumnLayout {
                    Layout.alignment: Qt.AlignTop
                    Layout.preferredWidth: 540
                    spacing: 32

                    Card {
                        name: "HWindow"
                        HWindow {
                            y: 10
                            width: 420
                            label: "fish: ~/about"
                            HText { Layout.alignment: Qt.AlignHCenter; text: "☼ universe ☼"; ink: "foam" }
                            HText { Layout.fillWidth: true; wrapMode: Text.Wrap; text: "Light glimmers from the stars and reflects upon the windows. A terminal hums here." }
                            HText { text: "It calls to you. <font color=\"" + HTokens.love + "\">The World Machine.</font>"; textFormat: Text.StyledText }
                            Item { implicitHeight: 16 }
                            PromptLine { path: "~/about"; command: "cat"; args: "details.md" }
                        }
                    }

                    Card {
                        name: "SettingRow / Segmented / Spin / Chip / Keycap footer"
                        HWindow {
                            y: 10
                            width: 520
                            label: "cat desktop.toml"
                            hints: [{ key: "space", text: "toggle" }, { key: "←→", text: "choose" }]
                            note: "saved to ~/.config/hecate/shell.toml"
                            HText { text: "background"; ink: "foam"; Layout.topMargin: 6; Layout.bottomMargin: 6 }
                            SettingRow {
                                Layout.fillWidth: true
                                name: "background"
                                description: "Blobs: the live simulation, the default."
                                Segmented { model: ["blobs", "wallpaper"]; currentIndex: 0 }
                            }
                            SettingRow {
                                Layout.fillWidth: true
                                enabled: false
                                name: "dither to haiiro"
                                description: "Wallpaper only."
                                Chip { text: "on" }
                            }
                            SettingRow {
                                Layout.fillWidth: true
                                name: "toast timeout"
                                Spin { value: 5; from: 1; to: 30; suffix: "s" }
                            }
                            SettingRow {
                                Layout.fillWidth: true
                                name: "badges"
                                Segmented { model: ["none", "count", "all"]; currentIndex: 1 }
                            }
                        }
                    }

                    Card {
                        name: "ListRow"
                        HWindow {
                            y: 10
                            width: 480
                            label: "launcher"
                            PromptLine { path: "~"; input: true; text: "fire" }
                            HRule { Layout.fillWidth: true }
                            HLabel { text: "apps" }
                            ListRow { Layout.fillWidth: true; selected: true; text: "Firefox"; match: "fire"; meta: "web browser"; Keycap { key: "enter" } }
                            ListRow { Layout.fillWidth: true; text: "Firewall"; match: "fire"; meta: "settings" }
                            ListRow { Layout.fillWidth: true; text: "~/notes/first-boot.md"; ink: "iris"; meta: "2 days ago" }
                        }
                    }
                }

                ColumnLayout {
                    Layout.alignment: Qt.AlignTop
                    Layout.preferredWidth: 400
                    spacing: 32

                    Card {
                        name: "HWindow love / gold / foam"
                        Column {
                            y: 10
                            spacing: 22
                            HWindow { width: 200; label: "notify: build"; variant: "love"; HText { text: "build failed"; ink: "love" } }
                            HWindow { width: 200; label: "claude: ~/ask"; variant: "gold"; HText { text: "needs you"; ink: "gold" } }
                            HWindow { width: 200; label: "notify: nix"; variant: "foam"; HText { text: "all checks passed"; ink: "foam" } }
                        }
                    }

                    Card {
                        name: "Slider"
                        Column {
                            width: 360
                            spacing: 18
                            Slider { width: 360; label: "volume"; value: 0.64; stepSize: 0.01; onMoved: v => value = v }
                            Slider { width: 360; label: "bright"; value: 0.45; stepSize: 0.01; onMoved: v => value = v }
                            Slider { width: 360; label: "osd"; value: 0.3; thumb: false }
                        }
                    }

                    Card {
                        name: "Progress"
                        Column {
                            width: 360
                            spacing: 10
                            Progress { width: 360; label: "Main story"; count: "7 / 9"; value: 0.78 }
                            Progress { width: 360; label: "Side missions"; count: "12 / 40"; value: 0.30 }
                        }
                    }

                    Card {
                        name: "CalendarGrid"
                        CalendarGrid { width: 300; year: 2026; month: 10; today: new Date(2026, 10, 19); eventDays: [21] }
                    }

                    Card {
                        name: "PromptLine"
                        Column {
                            spacing: 0
                            PromptLine { path: "~/about"; command: "cat"; args: "details.md" }
                            PromptLine { path: "~/about/endfield"; script: "./stats.sh" }
                            Item { width: 1; height: 16 }
                            HText { text: "The computer hums. It seems to be giving you a choice."; ink: "rose" }
                        }
                    }
                }

                ColumnLayout {
                    Layout.alignment: Qt.AlignTop
                    Layout.preferredWidth: 440
                    spacing: 32

                    Card {
                        name: "Dither / Veil"
                        Column {
                            spacing: 16
                            Row {
                                spacing: 16
                                Dither { width: 64; height: 64 }
                                Dither { width: 64; height: 64; tone: "lo" }
                                Dither { width: 64; height: 64; tone: "pine" }
                                Dither { width: 64; height: 64; big: true }
                            }
                            Rectangle {
                                width: 200
                                height: 64
                                color: HTokens.pine
                                Veil { anchors.fill: parent; heavy: true; blocking: false }
                                HText { x: 4; y: 4; text: "veil over pine" }
                            }
                        }
                    }

                    Card {
                        name: "InkButton"
                        Flow {
                            width: 400
                            spacing: 12
                            InkButton { text: "allow"; ink: "c3" }
                            InkButton { text: "always in this repo"; ink: "c5" }
                            InkButton { text: "deny"; ink: "c2" }
                            InkButton { text: "dismiss"; ink: "c1" }
                            InkButton { text: "go to terminal"; flat: true }
                        }
                    }

                    Card {
                        name: "Keycap"
                        Flow {
                            width: 400
                            spacing: 12
                            Keycap { key: "Mod+A"; hint: "launcher" }
                            Keycap { key: "Mod+/"; hint: "ask" }
                            Keycap { key: "enter"; hint: "go" }
                            Keycap { key: "ctrl enter"; hint: "run" }
                        }
                    }

                    Card {
                        name: "Chip / badge"
                        Row {
                            spacing: 8
                            Chip { text: "Stage 1"; on: true; anchors.verticalCenter: parent.verticalCenter }
                            Chip { text: "Stage 2"; on: true; anchors.verticalCenter: parent.verticalCenter }
                            Chip { text: "Stage 3"; anchors.verticalCenter: parent.verticalCenter }
                            Item { width: 8; height: 1 }
                            HText { text: "Umbral Monument"; role: "headingSm"; anchors.verticalCenter: parent.verticalCenter }
                            Chip { text: "new"; badge: true; anchors.verticalCenter: parent.verticalCenter }
                        }
                    }

                    Card {
                        name: "Transport"
                        Row {
                            spacing: 12
                            Transport { playing: true; anchors.verticalCenter: parent.verticalCenter }
                            Transport { playing: true; small: true; anchors.verticalCenter: parent.verticalCenter }
                            Transport { playing: false; small: true; anchors.verticalCenter: parent.verticalCenter }
                        }
                    }

                    Card {
                        name: "Spark"
                        Column {
                            width: 320
                            spacing: 16
                            Spark { values: [19, 23, 10, 29, 34, 13, 9, 8, 5, 29, 39, 22, 7, 18, 37, 38, 27, 21, 15, 10, 20, 17, 5, 20, 21, 16, 14, 23, 22, 27, 9, 25, 28, 36, 19, 15]; max: 40 }
                            Spark { ink: "iris"; values: [19, 34, 21, 9, 39, 23, 4, 22, 40, 23, 36, 16, 30, 31, 22, 31, 32, 14, 18, 23, 20, 6, 9, 6, 33, 21, 37, 38, 34, 25, 13, 16, 8, 30, 16, 32]; max: 40 }
                            Spark { width: 320; values: [20, 26, 30, 24, 18, 14]; max: 41; labels: ["22", "23", "00", "01", "02", "03"] }
                        }
                    }

                    Card {
                        name: "CodeBlock"
                        CodeBlock {
                            width: 420
                            lines: ["mkdir -p gen", "cp gen-presets/hecate.nix gen/cakeos.nix", "- bar.clock.seconds = false", "+ bar.clock.seconds = true"]
                        }
                    }
                }
            }

            HLabel { text: "free forms" }
            Grid {
                id: freeSheet
                objectName: "freeSheet"
                columns: 2
                spacing: 8

                FreeCard {
                    name: "FreeWidget"
                    FreeWidget {
                        width: 300
                        label: "uptime"
                        FreeText { text: "3h 12m"; role: "nav" }
                        FreeText { text: "up since 18:22 · load 0.42"; role: "code"; ink: "subtle" }
                    }
                }
                FreeCard {
                    name: "PixelCells"
                    Column {
                        PixelCells { count: 10; filled: 9 }
                        PixelCells { count: 48; filled: 46; ink: "gold"; width: 238 }
                        PixelCells { count: 25; filled: 11; large: true }
                        PixelCells { count: 25; filled: 5; large: true }
                    }
                }
                FreeCard {
                    name: "DayRuler"
                    implicitHeight: 140
                    DayRuler { hour: 21 }
                }
                FreeCard {
                    name: "RainColumns"
                    RainColumns {
                        width: 252
                        hours: [{ hour: "22", chance: 80 }, { hour: "23", chance: 70 }, { hour: "00", chance: 55 }, { hour: "01", chance: 40 },
                            { hour: "02", chance: 30 }, { hour: "03", chance: 30 }, { hour: "04", chance: 40 }]
                    }
                }
                FreeCard {
                    name: "MirrorSpark"
                    implicitHeight: 190
                    MirrorSpark {
                        downMax: 100
                        upMax: 100
                        down: [20, 35, 30, 60, 85, 70, 40, 45, 90, 100, 75, 50, 30, 25, 55, 65, 45, 30, 20, 15, 35, 60, 40, 25, 30, 45, 70, 55, 35, 20]
                        up: [10, 15, 10, 20, 30, 25, 15, 10, 40, 35, 20, 15, 10, 10, 20, 25, 15, 10, 10, 5, 15, 20, 15, 10, 10, 15, 25, 20, 15, 10]
                    }
                }
                FreeCard {
                    name: "PixelRing"
                    implicitHeight: 260
                    PixelRing {
                        done: 15
                        Column {
                            anchors.centerIn: parent
                            spacing: 4
                            FreeText { anchors.horizontalCenter: parent.horizontalCenter; text: "18:42"; role: "nav"; ink: "gold" }
                            FreeText { anchors.horizontalCenter: parent.horizontalCenter; text: "focus · 3 of 4"; role: "caption"; ink: "subtle" }
                        }
                    }
                }
                FreeCard {
                    name: "PixelBattery"
                    implicitHeight: 160
                    Row {
                        spacing: 16
                        PixelBattery { level: 0.87; anchors.verticalCenter: parent.verticalCenter }
                        FreeText { text: "87%"; role: "display"; size: 72 }
                    }
                }
                FreeCard {
                    name: "PixelMoon / SunPath"
                    implicitHeight: 220
                    Row {
                        spacing: 16
                        PixelMoon { illumination: 0.38; waxing: true }
                        SunPath { progress: 0.37 }
                    }
                }
                FreeCard {
                    name: "PixelBuds"
                    implicitHeight: 260
                    Row {
                        spacing: 40
                        PixelBuds { part: "left"; anchors.bottom: parent.bottom }
                        PixelBuds { part: "case"; anchors.bottom: parent.bottom }
                        PixelBuds { part: "right"; anchors.bottom: parent.bottom }
                        PixelBuds { part: "max"; anchors.bottom: parent.bottom }
                        PixelBuds { part: "left"; dim: true; anchors.bottom: parent.bottom }
                    }
                }
                FreeCard {
                    name: "PixelGlyph"
                    Row {
                        spacing: 36
                        PixelGlyph { name: "cloud"; anchors.verticalCenter: parent.verticalCenter }
                        PixelGlyph { name: "bell"; anchors.verticalCenter: parent.verticalCenter }
                        PixelGlyph { name: "toggleOff"; anchors.verticalCenter: parent.verticalCenter }
                        PixelGlyph { name: "toggleOn"; anchors.verticalCenter: parent.verticalCenter }
                        PixelGlyph { name: "clipboard"; anchors.verticalCenter: parent.verticalCenter }
                    }
                }
                FreeCard {
                    name: "GlyphTransport / PlayLine"
                    Column {
                        width: 300
                        Item { width: 1; height: 6 }
                        PlayLine { width: parent.width; progress: 0.46 }
                        GlyphTransport { x: -12; playing: true; time: "1:52 / 4:03" }
                    }
                }
                FreeCard {
                    name: "RuledNote"
                    RuledNote {
                        width: 260
                        endCursor: true
                        text: "tea: sencha, 3 min\n\nask kuro about the\nminecraft backup"
                    }
                }
                FreeCard {
                    name: "PixelCheck / TextBar / BlockCursor / FreeArt"
                    implicitHeight: 260
                    Row {
                        spacing: 32
                        Column {
                            spacing: 2
                            Repeater {
                                model: [{ text: "greetd fix", done: true }, { text: "iso build", done: true }, { text: "bar mock", done: false }, { text: "groceries", done: false }]
                                Row {
                                    id: todo
                                    required property var modelData
                                    required property int index
                                    spacing: 10
                                    FreeText { width: 16; horizontalAlignment: Text.AlignRight; height: 24; text: todo.index + 1; role: "caption"; ink: "highlightHigh" }
                                    PixelCheck { height: 24; checked: todo.modelData.done }
                                    FreeText { height: 24; text: todo.modelData.text; ink: todo.modelData.done ? "subtle" : "text"; font.strikeout: todo.modelData.done }
                                }
                            }
                            FreeText { leftPadding: 16; text: "~"; ink: "pine" }
                        }
                        Column {
                            spacing: 2
                            Row { spacing: 8; FreeText { text: "cpu"; ink: "subtle" } TextBar { value: 0.23 } FreeText { text: "23%"; ink: "foam" } }
                            Row { spacing: 8; FreeText { text: "ram"; ink: "subtle" } TextBar { value: 0.35 } FreeText { text: "35%"; ink: "foam" } }
                            Row { spacing: 8; FreeText { text: "gpu"; ink: "subtle" } TextBar { value: 0.08 } FreeText { text: " 8%"; ink: "foam" } }
                            Item { width: 1; height: 16 }
                            Row {
                                FreeText { text: "The tea has gone cold."; role: "nav"; ink: "rose" }
                                Item { width: 8; height: 1 }
                                BlockCursor { ink: "rose"; size: 32; anchors.verticalCenter: parent.verticalCenter }
                            }
                            Item { width: 1; height: 16 }
                            FreeArt { width: 64; height: 64; Dither { anchors.fill: parent; tone: "pine"; big: true } }
                        }
                    }
                }
            }

            Card {
                name: "PixelTransition (click to replay)"
                Row {
                    y: 10
                    spacing: 24
                    Repeater {
                        model: 3
                        PixelTransition {
                            required index
                            shown: root.shown
                            HWindow {
                                width: 240
                                label: "kitty: ~/src/hecate"
                                PromptLine { path: "~/src"; command: "nix"; args: "flake check" }
                                HText { text: "all checks passed"; ink: "foam" }
                            }
                        }
                    }
                    InkButton {
                        anchors.verticalCenter: parent.verticalCenter
                        text: root.shown ? "close" : "open"
                        ink: root.shown ? "c2" : "c5"
                        onClicked: root.shown = !root.shown
                    }
                }
            }

            Card {
                name: "Type and glyph fallback"
                Layout.fillWidth: true
                Column {
                    spacing: 6
                    Row {
                        spacing: 24
                        HText { text: "universe"; role: "display" }
                        Column {
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 4
                            HText { text: "about  blog  projects"; role: "nav"; ink: "pine" }
                            HText { text: "8799 / 9131"; role: "valueLg" }
                            HText { text: "Hello. I'm cakes, creator of this website."; bold: true }
                            HText { text: "Light glimmers from the stars and reflects upon the windows." }
                            HText { text: "missions"; role: "headingSm" }
                            HText { text: "Dungeon III — cleared"; role: "small" }
                            HText { text: "last played"; role: "label"; ink: "inkMuted" }
                            HText { text: "new"; role: "badge" }
                        }
                    }
                    HText { text: "latin   ÀÉÎÕÜ ß åæø Łąęśćźż Œœ Šž ăț őű ığş ẞ" }
                    HText { text: "symbols ☼ » « @ ★ ☆ ✓ ✗ ‹ › ← ↑ → ↓ ↵ ⌘ ⌥ € £ ¥ © ° ± × ÷ … — • ♪ ▶ ⏸ ≡" }
                    HText { text: "boxes   ┌─┬─┐ │ ├─┼─┤ └─┴─┘ ╔═╦═╗ ║ ╚═╩═╝ ░▒▓█ ▀▄▌▐" }
                    HText { text: "cyrillic Съешь же ещё этих мягких французских булок Ґґ Єє Іі Її" }
                    HText { text: "other   αβγ ΔΩ 灰色 はいいろ हिन्दी שלום" }
                }
            }
        }
    }
}
