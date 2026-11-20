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
                            Slider { width: 360; label: "volume"; value: 0.64; stepSize: 0.01 }
                            Slider { width: 360; label: "bright"; value: 0.45; stepSize: 0.01 }
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
