pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.modules.common.widgets.haiiro

// A tray item's menu as a window under its icon: one ListRow per entry with
// the rose » on the row under the pointer, hairline rules for separators,
// quit and exit in love. An entry with a submenu opens it in place, with a
// "‹ back" row on top.
Scope {
    id: root

    required property Item target
    property QsMenuHandle handle: null
    property string label: ""
    property bool wanted: false
    readonly property var popupWindow: window.popupWindow
    signal done()

    // the submenus walked into, innermost last
    property list<QsMenuHandle> trail: []
    readonly property QsMenuHandle current: root.trail.length > 0 ? root.trail[root.trail.length - 1] : root.handle

    onWantedChanged: if (!root.wanted) root.trail = []

    BarPopup {
        id: window
        target: root.target
        wanted: root.wanted
        label: root.label
        windowWidth: 240
        content: Component {
            ColumnLayout {
                id: list
                spacing: 0

                QsMenuOpener {
                    id: opener
                    menu: root.current
                }

                ListRow {
                    Layout.fillWidth: true
                    visible: root.trail.length > 0
                    text: "‹ back"
                    ink: "muted"
                    selected: hovered
                    onClicked: root.trail = root.trail.slice(0, -1)
                }

                Repeater {
                    model: opener.children

                    Item {
                        id: entry
                        required property QsMenuEntry modelData
                        readonly property bool quits: /\b(quit|exit)\b/i.test(entry.modelData.text)
                        Layout.fillWidth: true
                        implicitHeight: entry.modelData.isSeparator ? rule.implicitHeight : row.implicitHeight
                        visible: entry.modelData.isSeparator || entry.modelData.text.length > 0

                        HRule {
                            id: rule
                            width: parent.width
                            visible: entry.modelData.isSeparator
                        }

                        ListRow {
                            id: row
                            width: parent.width
                            visible: !entry.modelData.isSeparator
                            enabled: entry.modelData.enabled
                            opacity: enabled ? 1 : 0.55
                            selected: hovered && enabled
                            text: (entry.modelData.checkState === Qt.Checked ? "✓ " : "") + entry.modelData.text.replace(/_(?!_)/g, "") + (entry.modelData.hasChildren ? " ›" : "")
                            ink: entry.quits ? "love" : "text"
                            onClicked: {
                                if (entry.modelData.hasChildren) {
                                    root.trail = root.trail.concat([entry.modelData]);
                                } else {
                                    entry.modelData.triggered();
                                    root.done();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
