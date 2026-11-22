import QtQuick
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro as H
import "../desk/SkyMath.js" as SkyMath

// `sky`: a pixel moon at its real phase, and the sun's path under (or over)
// the horizon with a gold dot where the sun is now.
H.FreeWidget {
    id: root

    label: "sky"

    SystemClock {
        id: clock
        enabled: root.live
        precision: SystemClock.Minutes
    }
    // `at` overrides the date, for checking the moon
    property var at: null
    readonly property date now: at ?? clock.date
    readonly property var moon: SkyMath.moon(now)
    readonly property var sky: SkyMath.sky(now, Weather.location)
    readonly property bool waxing: moon.age < SkyMath.SYNODIC / 2
    // 0-1 along the arc: sunrise to sunset by day, sunset to sunrise by night
    readonly property real progress: {
        const t = now.getTime();
        const day = d => SkyMath.sunDay(new Date(now.getFullYear(), now.getMonth(), now.getDate() + d, 12), Weather.location);
        const today = day(0);
        const rise = today.rise.getTime(), set = today.set.getTime();
        let from, to;
        if (t >= rise && t < set) { from = rise; to = set; }
        else if (t < rise) { from = day(-1).set.getTime(); to = rise; }
        else { from = set; to = day(1).rise.getTime(); }
        return Math.max(0, Math.min(1, (t - from) / (to - from)));
    }

    art: H.PixelMoon {
        illumination: root.moon.illum
        waxing: root.waxing
    }

    Row {
        spacing: 8
        H.FreeText { role: "body"; text: root.moon.name }
        H.FreeText { role: "body"; ink: "subtle"; text: Math.round(root.moon.illum * 100) + "%" }
    }
    H.SunPath {
        progress: root.progress
        up: root.sky.isDay
    }
    Row {
        spacing: 32
        H.FreeText { role: "code"; ink: "subtle"; text: "set " + SkyMath.clock(root.sky.nextSet) }
        H.FreeText { role: "code"; ink: "subtle"; text: "rise " + SkyMath.clock(root.sky.nextRise) }
    }
    H.FreeText {
        role: "code"
        ink: "subtle"
        text: "hour of " + root.sky.planet + " until " + SkyMath.clock(root.sky.planetUntil)
    }
}
