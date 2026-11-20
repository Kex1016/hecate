pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro

// Updates waiting, in gold ("↑3"), shown only when there are some. Hover:
// `git log` with the commits not installed yet; click: the updater.
BarSegment {
    id: root

    readonly property var commits: Updates.cakeosStatus?.commits ?? []

    visible: Updates.count > 0
    popupLabel: Updates.cakeos ? "git log" : "updates"
    popupWidth: 360
    popupVariant: "gold"
    onClicked: if (Updates.cakeos) Updates.openUpdater()

    HText {
        text: "↑" + Updates.count
        ink: "gold"
    }

    popup: Component {
        ColumnLayout {
            spacing: 0

            Repeater {
                model: root.commits.slice(0, 6)

                Row {
                    id: commit
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: HTokens.ch()

                    HText {
                        text: String(commit.modelData.sha ?? "").slice(0, 7)
                        ink: "gold"
                    }
                    HText {
                        width: commit.width - x
                        text: commit.modelData.subject ?? ""
                        elide: Text.ElideRight
                    }
                }
            }
            HText {
                Layout.fillWidth: true
                Layout.topMargin: root.commits.length > 0 ? HTokens.space2 : 0
                text: Updates.count + (Updates.cakeos ? (Updates.count === 1 ? " commit" : " commits") : (Updates.count === 1 ? " package" : " packages")) + " to install"
                ink: "inkMuted"
                role: "caption"
            }
        }
    }
}
