import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H

// A heading inside a settings page: one foam word. The first in a column sits
// 6px down, later ones 14px.
//   SetSection { text: "background" }
H.HText {
    id: root

    Layout.fillWidth: true
    Layout.topMargin: root.parent && root.parent.children[0] === root ? 6 : 14
    Layout.bottomMargin: 6
    ink: "foam"
}
