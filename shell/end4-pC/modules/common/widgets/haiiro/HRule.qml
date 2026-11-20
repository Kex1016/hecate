import QtQuick
import qs.modules.common.widgets.haiiro

// The `.hr` divider: a 1px hairline with 6px above and below.
Item {
    implicitHeight: 13
    implicitWidth: 1

    Rectangle {
        y: 6
        width: parent.width
        height: HTokens.borderHairline
        color: HTokens.inkHairline
    }
}
