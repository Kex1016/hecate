import qs.modules.common.widgets.haiiro as H
import QtQuick

// A toggle chip that fills its grid cell: solid with a check when on, dashed
// when off. Click or space toggles. A chip with a `list` opens it in place on
// a long press, or with shift+enter / the menu key.
H.Chip {
    id: root

    // the QuickState view a long press opens, or ""
    property string list: ""
    signal toggled()
    signal listRequested()

    activeFocusOnTab: enabled
    implicitHeight: 2 * H.HTokens.borderHairline + Math.round(H.HTokens.fontLabel * 1.6)

    Keys.onPressed: event => {
        const enter = event.key === Qt.Key_Return || event.key === Qt.Key_Enter;
        if (root.list !== "" && (event.key === Qt.Key_Menu || (enter && (event.modifiers & Qt.ShiftModifier)))) {
            root.listRequested();
            event.accepted = true;
        } else if (event.key === Qt.Key_Space || enter) {
            root.toggled();
            event.accepted = true;
        }
    }

    hoverable: true
    onClicked: root.toggled()
    onPressAndHold: {
        if (root.list !== "")
            root.listRequested();
    }
}
