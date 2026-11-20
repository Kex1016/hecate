import QtQuick
import qs.modules.common.widgets.haiiro as H

// A read-only row: a name and what it is.
//   SetInfo { name: "kernel"; text: SystemInfo.kernelVersion }
SetRow {
    id: root

    property string text: ""
    property string ink: "subtle"

    H.HText {
        text: root.text || "…"
        ink: root.ink
        elide: Text.ElideLeft
        width: Math.min(implicitWidth, root.width * 0.6)
    }
}
