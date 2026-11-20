import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro as H
import "SkyMath.js" as SkyMath

// `sky`: the moon's phase and the planetary hour, worked out from the date
// and, for sunrise and sunset, the weather service's location.
H.HWindow {
    id: root

    property bool live: true
    SystemClock {
        id: clock
        enabled: root.live
        precision: SystemClock.Minutes
    }
    readonly property var moon: SkyMath.moon(clock.date)
    readonly property var sky: SkyMath.sky(clock.date, Weather.location)

    widget: true
    label: "sky"
    implicitWidth: 360

    RowLayout {
        spacing: 0
        H.HText { ink: "c3"; text: SkyMath.moonGlyph(root.moon.name) + " " }
        H.HText { text: root.moon.name }
        H.HText { ink: "c1"; text: " · " + Math.round(root.moon.illum * 100) + "%" }
    }
    RowLayout {
        spacing: 0
        H.HText { ink: "c3"; text: "☼ " }
        H.HText { text: root.sky.isDay ? "set " + SkyMath.clock(root.sky.nextSet) : "rise " + SkyMath.clock(root.sky.nextRise) }
        H.HText { ink: "c1"; text: " · " + (root.sky.isDay ? "rise " + SkyMath.clock(root.sky.nextRise) : "set " + SkyMath.clock(root.sky.nextSet)) }
    }
    H.HText { ink: "c1"; text: "hour of " + root.sky.planet + " until " + SkyMath.clock(root.sky.planetUntil) }
    H.HText {
        ink: "c4"
        Layout.fillWidth: true
        wrapMode: Text.WordWrap
        text: SkyMath.MOON_LINES[root.moon.name] ?? ""
    }
}
