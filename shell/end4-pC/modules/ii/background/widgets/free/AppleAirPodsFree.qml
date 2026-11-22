pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services.apple
import qs.modules.common.widgets.haiiro as H

// airpods, free · the buds and the case drawn in pixels, each with its charge under it as a
// number and ten cells. A bud at 20% or under turns love; one out of the ear dims, lifts and
// reads "out". The Max is one headset and one level in Jacquard. Not connected: the three
// outlines in highlight and the one thing to do. From librepods (services/apple/AirPods.qml).
H.FreeWidget {
    id: root

    // "AirPods Pro" -> "pro", "AirPods Max" -> "max", "AirPods (3rd generation)" -> "3"
    readonly property string model: {
        const name = AirPods.name.toLowerCase();
        const m = name.match(/airpods\s*(pro|max|\(?\d)/);
        if (m) return m[1].replace("(", "");
        return name.length > 0 ? name : "airpods";
    }
    readonly property real lowAt: 20
    readonly property bool leftOut: AirPods.leftState === "out"
    readonly property bool rightOut: AirPods.rightState === "out"
    readonly property bool lowBud: AirPods.connected && !AirPods.isHeadset
        && ((AirPods.left >= 0 && AirPods.left <= root.lowAt && !root.leftOut) || (AirPods.right >= 0 && AirPods.right <= root.lowAt && !root.rightOut))
    readonly property string lowSide: (AirPods.left >= 0 && AirPods.left <= root.lowAt) ? "left" : "right"

    label: AirPods.connected ? `airpods: ${root.model}` : "airpods"
    implicitWidth: 420

    function percent(v) {
        return v >= 0 ? `${v}%` : "--";
    }

    // one part: the picture lifted on its own column, the figure, ten cells and a caption
    component Part: ColumnLayout {
        id: part
        property string part: "left"
        property int value: -1
        property bool out: false
        property string ink: "foam"
        property string caption: ""
        readonly property bool low: value >= 0 && value <= root.lowAt && !out

        spacing: 6
        Layout.alignment: Qt.AlignTop

        Item {
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: art.implicitWidth
            Layout.preferredHeight: 108
            H.PixelBuds {
                id: art
                part: part.part
                dim: part.out
                // the case sits on the floor; with a bud out, the bud still in drops below the lifted one
                y: part.part === "case" ? 108 - implicitHeight - 4 : (!part.out && (root.leftOut || root.rightOut) ? 12 : 0)
            }
        }
        H.FreeText {
            Layout.alignment: Qt.AlignHCenter
            role: "nav"
            ink: part.out ? "muted" : part.low ? "love" : part.part === "case" ? "gold" : "text"
            text: part.out ? "out" : root.percent(part.value)
        }
        H.PixelCells {
            Layout.alignment: Qt.AlignHCenter
            count: 10
            filled: part.value < 0 ? 0 : Math.round(part.value / 10)
            ink: part.out ? "text" : part.low ? "love" : part.part === "case" ? "gold" : part.ink
        }
        H.FreeText {
            Layout.alignment: Qt.AlignHCenter
            role: "caption"
            ink: "subtle"
            text: part.caption
        }
    }

    // buds and case, spread over the width
    RowLayout {
        visible: AirPods.connected && !AirPods.isHeadset
        spacing: 36
        Part {
            part: "left"
            value: AirPods.left
            out: root.leftOut
            caption: root.leftOut && AirPods.left >= 0 ? `left · ${AirPods.left}%` : "left"
        }
        Part {
            part: "case"
            value: AirPods.caseLevel
            ink: "gold"
            visible: !(root.leftOut || root.rightOut)
            caption: "case"
        }
        Part {
            part: "right"
            value: AirPods.right
            out: root.rightOut
            caption: root.rightOut && AirPods.right >= 0 ? `right · ${AirPods.right}%` : "right"
        }
    }

    // AirPods Max: one headset, one level in Jacquard
    RowLayout {
        visible: AirPods.connected && AirPods.isHeadset
        spacing: 24
        H.PixelBuds { part: "max" }
        ColumnLayout {
            spacing: 6
            H.FreeText {
                role: "display"
                size: 48
                text: root.percent(AirPods.headset)
            }
            H.PixelCells {
                count: 10
                filled: AirPods.headset < 0 ? 0 : Math.round(AirPods.headset / 10)
                ink: AirPods.headset >= 0 && AirPods.headset <= root.lowAt ? "love" : "foam"
            }
        }
    }

    H.FreeText {
        visible: root.lowBud
        role: "caption"
        ink: "love"
        text: `${root.lowSide} bud low`
    }

    // the four noise modes as ink words, the current one in pine
    Row {
        visible: AirPods.connected || AirPods.external
        Layout.topMargin: 4
        spacing: 8
        Repeater {
            model: AirPods.modes
            delegate: Item {
                id: mode
                required property var modelData
                readonly property bool on: AirPods.noiseMode === modelData.id
                width: label.implicitWidth + 16
                height: label.implicitHeight + 4
                Rectangle {
                    anchors.fill: parent
                    visible: mode.on
                    color: H.HTokens.pine
                }
                H.FreeText {
                    id: label
                    anchors.centerIn: parent
                    role: "code"
                    ink: mode.on ? "text" : "muted"
                    shadow: mode.on ? "none" : "free"
                    text: mode.modelData.label
                }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: AirPods.setNoiseMode(mode.modelData.id)
                }
            }
        }
    }

    H.FreeText {
        visible: AirPods.connected && !root.leftOut && !root.rightOut
        role: "caption"
        ink: "subtle"
        text: (AirPods.pauseOnRemove ? "take one out to pause" : "taking one out keeps playing") + " · the bar shows the lower bud"
    }
    // the narrator's line while a bud is out
    H.FreeText {
        Layout.fillWidth: true
        visible: AirPods.connected && (root.leftOut || root.rightOut) && AirPods.pauseOnRemove
        wrapMode: Text.Wrap
        ink: "rose"
        text: "The music waits until it is back in."
    }

    // not connected: the three outlines, smaller, and what is missing in one line
    RowLayout {
        visible: !AirPods.connected
        spacing: 12
        H.PixelBuds { part: "left"; cell: 4; dim: true; Layout.alignment: Qt.AlignBottom }
        H.PixelBuds { part: "case"; cell: 4; dim: true; Layout.alignment: Qt.AlignBottom }
        H.PixelBuds { part: "right"; cell: 4; dim: true; Layout.alignment: Qt.AlignBottom }
    }
    H.FreeText {
        Layout.fillWidth: true
        visible: !AirPods.connected
        role: "code"
        ink: "muted"
        wrapMode: Text.Wrap
        text: !AirPods.installed ? "librepods is not installed · cakeos.hardware airpods"
            : AirPods.external ? "librepods is running on its own, so the battery can't be read"
            : !AirPods.wanted ? "airpods are off in settings"
            : "not connected · open the case near this computer"
    }

    RowLayout {
        visible: AirPods.external || AirPods.running
        spacing: 8
        H.InkButton {
            visible: AirPods.external
            ink: "c3"
            role: "code"
            text: "show battery"
            onClicked: AirPods.takeOver()
        }
        H.InkButton {
            ink: "c1"
            role: "code"
            text: "librepods"
            onClicked: AirPods.openApp()
        }
    }
}
