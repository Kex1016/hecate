import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H

// A paragraph inside a page: plain 14px text, or the narrator's rose line.
H.HText {
    property bool narrator: false

    Layout.fillWidth: true
    Layout.topMargin: 4
    Layout.bottomMargin: 4
    role: narrator ? "body" : "code"
    ink: narrator ? "rose" : "text"
    wrapMode: Text.Wrap
}
