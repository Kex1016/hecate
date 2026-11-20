import qs
import qs.services
import qs.services.claude
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import qs.modules.ii.sidebarRight.lists
import qs.modules.ii.sidebarRight.notifications
import qs.modules.ii.sidebarRight.tabs
import QtQuick
import QtQuick.Layouts

// The quick panel's one layout (QuickPanel board): who and how long, the
// toggle chips, the sliders, notifications, then calendar, todo and timer
// tabs at the bottom. A long-pressed chip opens its list in place of the
// sliders and notifications; esc or « back returns.
H.HWindow {
    id: root

    label: "hecate: ~/quick"

    ColumnLayout {
        Layout.fillWidth: true
        Layout.fillHeight: true
        spacing: 14

        QuickHeader {
            Layout.fillWidth: true
        }

        QuickChips {
            Layout.fillWidth: true
        }

        // the panel itself
        ColumnLayout {
            visible: QuickState.view === ""
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 14

            QuickSliders {
                Layout.fillWidth: true
            }

            H.HRule {
                Layout.fillWidth: true
            }

            // With Claude on: after an unlock, what came in while the screen was locked
            // (ClaudeDigestWidget; it sends nothing until its own press).
            Loader {
                Layout.fillWidth: true
                active: Config.options.claude.enable && ClaudeBrief.lockedAt > 0
                    && Notifications.list.some(n => n.time >= ClaudeBrief.lockedAt)
                visible: active && status === Loader.Ready
                source: active ? Qt.resolvedUrl("../background/widgets/ClaudeDigestWidget.qml") : ""
            }

            NotificationList {
                Layout.fillWidth: true
                Layout.fillHeight: true
            }
        }

        // or the list a chip opened
        Loader {
            active: QuickState.view !== ""
            visible: active
            Layout.fillWidth: true
            Layout.fillHeight: true
            sourceComponent: QuickList {
                view: QuickState.view
            }
        }

        QuickTabs {
            Layout.fillWidth: true
        }
    }
}
