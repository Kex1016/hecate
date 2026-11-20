pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common
import qs.modules.ii.settings

SettingsPage {
    id: page

    readonly property var aliases: Config.options.launcher.aliases ?? ({})
    readonly property var aliasNames: Object.keys(page.aliases).sort()

    function setAliases(next) {
        Config.options.launcher.aliases = next;
    }

    SetColumn {
        SetSection { text: "launcher" }
        SetNote {
            text: "Your own names for apps. Typing an alias exactly puts its app first, and the row says which alias matched (yazi · files)."
        }
        Repeater {
            model: page.aliasNames
            SetAction {
                required property string modelData
                name: modelData
                description: "opens " + page.aliases[modelData]
                buttons: [{ text: "remove", ink: "c2" }]
                onPressed: {
                    const next = Object.assign({}, page.aliases);
                    delete next[modelData];
                    page.setAliases(next);
                }
            }
        }
        SetText {
            name: "new alias"
            description: "alias = app, then enter: files = yazi"
            placeholder: "files = yazi"
            value: ""
            clearOnCommit: true
            onEdited: v => {
                const m = String(v).match(/^\s*([^=\s]+)\s*=\s*(.+?)\s*$/);
                if (!m) return;
                const next = Object.assign({}, page.aliases);
                next[m[1]] = m[2];
                page.setAliases(next);
            }
        }
        SetSection { text: "search" }
        SetText {
            name: "web search"
            description: "The query goes after this address."
            key: "search.engineBaseUrl"
            columns: 30
        }
        SetToggle {
            name: "sloppy matching"
            description: "Scores by edit distance instead of fuzzy order. Finds typos, and odd things."
            key: "search.sloppy"
        }
        SetText {
            name: "translate to"
            description: "The tr prefix and Mod+Shift+T. auto follows your locale."
            key: "language.translator.targetLanguage"
            columns: 10
        }
    }
    SetColumn {
        SetSection { text: "osd" }
        SetNumber {
            name: "osd timeout"
            key: "osd.timeout"
            from: 0.5
            to: 5
            stepSize: 0.5
            divisor: 1000
            suffix: "s"
        }
        SetSection { text: "screenshots" }
        SetText {
            name: "save shots to"
            description: "Empty: only copy them to the clipboard."
            key: "screenSnip.savePath"
            placeholder: "clipboard only"
            columns: 30
        }
        SetToggle {
            name: "annotate with satty"
            key: "regionSelector.annotation.useSatty"
        }
        SetToggle {
            name: "snap to windows"
            key: "regionSelector.targetRegions.windows"
        }
        SetSection { text: "recording" }
        SetText {
            name: "save recordings to"
            key: "screenRecord.savePath"
            columns: 30
        }
        SetToggle {
            name: "system audio"
            key: "screenRecord.systemAudio"
        }
        SetToggle {
            name: "microphone"
            key: "screenRecord.microphone"
        }
    }
}
