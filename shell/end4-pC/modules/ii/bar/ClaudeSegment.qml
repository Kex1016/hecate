pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common
import qs.modules.common.widgets.haiiro
import qs.modules.ii.claude
import qs.services.claude

// "claude", with a gold count when sessions wait for an answer ("claude 2").
// Only while Claude is switched on (claude.enable). Hover: the sessions and
// what each is doing; click: the sessions panel (Mod+Shift+/). A fullscreen
// window hides the permission toasts, so this count is how you see them.
BarSegment {
    id: root

    readonly property int waiting: SessionsState.waiting
    readonly property var live: (ClaudeSessions.sessions ?? []).filter(s => !s.ended)

    visible: Config.options.claude.enable && Config.options.claude.bridge.enable
    popupLabel: "claude: ~/sessions"
    popupVariant: root.waiting > 0 ? "gold" : "pine"
    popupWidth: 320
    onClicked: SessionsState.open = !SessionsState.open

    HText {
        text: "claude"
        ink: root.waiting > 0 ? "text" : "muted"
    }
    HText {
        visible: root.waiting > 0
        text: root.waiting
        ink: "gold"
    }

    popup: Component {
        ColumnLayout {
            spacing: HTokens.space1

            HText {
                visible: root.live.length === 0
                Layout.fillWidth: true
                text: "no sessions running"
                ink: "muted"
            }
            Repeater {
                model: root.live.slice(0, 5)
                RowLayout {
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: HTokens.ch()
                    HText {
                        Layout.fillWidth: true
                        text: modelData.dir || modelData.name || "claude"
                        ink: "foam"
                        elide: Text.ElideLeft
                    }
                    HText {
                        text: modelData.state === "waiting" ? "waiting" : modelData.state ?? ""
                        ink: modelData.state === "waiting" ? "gold" : modelData.state === "working" ? "iris" : "muted"
                    }
                }
            }
        }
    }
}
