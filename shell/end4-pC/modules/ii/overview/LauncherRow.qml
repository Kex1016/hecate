pragma ComponentBehavior: Bound
import QtQuick
import qs.services
import qs.modules.common.widgets.haiiro as H

// One launcher row from LauncherSearch: a kit ListRow with the Keycap on
// the selected row. LauncherLeadRow adds things before the text.
H.ListRow {
    id: root

    required property var entry

    text: entry.image ? "" : (entry.verb ? entry.text.slice(entry.verb.length + 1) : entry.text)
    match: entry.match ?? ""
    meta: entry.meta ?? ""
    ink: entry.ink ?? "text"

    H.Keycap {
        visible: root.selected && !root.entry.placeholder
        key: "enter"
    }
}
