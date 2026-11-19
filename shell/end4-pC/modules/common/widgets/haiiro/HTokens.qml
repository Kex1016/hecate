pragma Singleton
pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import qs.modules.common

// The one place the kit reads haiiro tokens. Every value comes from
// Appearance.haiiro (W0-theme); the literal after ?? is the tokens.json value,
// only used while Appearance does not expose the token yet.
Singleton {
    id: root

    readonly property var src: Appearance.haiiro ?? null
    readonly property var c: src?.color ?? null
    readonly property var f: src?.font ?? null
    readonly property var fs: src?.fontSize ?? null
    readonly property var sp: src?.space ?? null
    readonly property var bw: src?.border ?? null
    readonly property var sz: src?.size ?? null
    readonly property var tr: src?.transition ?? null

    // colour
    readonly property color base: c?.base ?? "#191724"
    readonly property color surface: c?.surface ?? "#1f1d2e"
    readonly property color overlay: c?.overlay ?? "#26233a"
    readonly property color muted: c?.muted ?? "#6e6a86"
    readonly property color subtle: c?.subtle ?? "#908caa"
    readonly property color text: c?.text ?? "#e0def4"
    readonly property color love: c?.love ?? "#eb6f92"
    readonly property color gold: c?.gold ?? "#f6c177"
    readonly property color rose: c?.rose ?? "#ebbcba"
    readonly property color pine: c?.pine ?? "#31748f"
    readonly property color foam: c?.foam ?? "#9ccfd8"
    readonly property color iris: c?.iris ?? "#c4a7e7"
    readonly property color highlightLow: c?.highlightLow ?? "#21202e"
    readonly property color highlightMed: c?.highlightMed ?? "#403d52"
    readonly property color highlightHigh: c?.highlightHigh ?? "#524f67"
    readonly property color inkMuted: c?.inkMuted ?? Qt.rgba(224 / 255, 222 / 255, 244 / 255, 0.55)
    readonly property color inkHairline: c?.inkHairline ?? Qt.rgba(224 / 255, 222 / 255, 244 / 255, 0.25)
    readonly property color inkTrack: c?.inkTrack ?? Qt.rgba(224 / 255, 222 / 255, 244 / 255, 0.14)
    readonly property color veilHeavy: c?.veilHeavy ?? Qt.rgba(25 / 255, 23 / 255, 36 / 255, 0.55)

    // Ink classes c1-c8, or a token name ("foam", "inkMuted"). Anything else passes through.
    function ink(name) {
        switch (name) {
        case "c1": case "muted": return root.muted
        case "c2": case "love": return root.love
        case "c3": case "gold": return root.gold
        case "c4": case "rose": return root.rose
        case "c5": case "pine": return root.pine
        case "c6": case "foam": return root.foam
        case "c7": case "iris": return root.iris
        case "c8": case "base": return root.base
        case "surface": return root.surface
        case "overlay": return root.overlay
        case "subtle": return root.subtle
        case "text": case "": case undefined: return root.text
        case "inkMuted": return root.inkMuted
        case "highlightMed": return root.highlightMed
        case "highlightHigh": return root.highlightHigh
        default: return name
        }
    }

    // type. QML takes one family per Text; the fallback stack (glyphs the
    // VGA face lacks) is resolved by fontconfig from W0-theme's font setup.
    // Bold is the same family with font.bold (the 2x face is its weight 700).
    readonly property string mono: f?.mono ?? "IBM VGA 8x16"
    readonly property string display: f?.display ?? "Jacquard 24"
    readonly property real lineHeight: f?.lineHeight ?? 1.15
    readonly property var ls: src?.letterSpacing ?? null
    readonly property real trackHeadingSm: ls?.headingSm ?? 1.12
    readonly property real trackLabel: ls?.label ?? 0.96
    readonly property real trackBadge: ls?.badge ?? 0.66

    readonly property int fontDisplay: fs?.display ?? 150
    readonly property int fontNav: fs?.nav ?? 32
    readonly property int fontValueLg: fs?.valueLg ?? 20
    readonly property int fontBody: fs?.body ?? 16
    readonly property int fontHeadingSm: fs?.headingSm ?? 14
    readonly property int fontSmall: fs?.small ?? 13
    readonly property int fontLabel: fs?.label ?? 12
    readonly property int fontBadge: fs?.badge ?? 11

    // One text role: pixel size, tracking (px) and whether it is uppercase.
    // code and caption are the 14px and 12px sizes without the uppercase.
    function role(name) {
        switch (name) {
        case "display": return { size: root.fontDisplay, tracking: 0, caps: false }
        case "nav": return { size: root.fontNav, tracking: 0, caps: false }
        case "valueLg": return { size: root.fontValueLg, tracking: 0, caps: false }
        case "headingSm": return { size: root.fontHeadingSm, tracking: root.trackHeadingSm, caps: true }
        case "code": return { size: root.fontHeadingSm, tracking: 0, caps: false }
        case "small": return { size: root.fontSmall, tracking: 0, caps: false }
        case "label": return { size: root.fontLabel, tracking: root.trackLabel, caps: true }
        case "caption": return { size: root.fontLabel, tracking: 0, caps: false }
        case "badge": return { size: root.fontBadge, tracking: root.trackBadge, caps: true }
        default: return { size: root.fontBody, tracking: 0, caps: false }
        }
    }
    // Line boxes are whole pixels so the bitmap face never lands between rows.
    function lineBox(size) { return Math.round(size * root.lineHeight) }
    // 1ch of the VGA face: glyphs are half as wide as they are tall.
    function ch(size) { return Math.round((size ?? root.fontBody) / 2) }

    // space
    readonly property int space1: sp?.s1 ?? 4
    readonly property int space2: sp?.s2 ?? 8
    readonly property int space3: sp?.s3 ?? 12
    readonly property int space4: sp?.s4 ?? 16
    readonly property int space6: sp?.s6 ?? 24
    readonly property int space8: sp?.s8 ?? 32
    readonly property int space12: sp?.s12 ?? 50
    readonly property int space20: sp?.s20 ?? 80

    // borders
    readonly property int borderHairline: bw?.hairline ?? 1
    readonly property int borderDivider: bw?.divider ?? 3
    readonly property int borderWindow: bw?.window ?? 5

    // fixed shell sizes
    readonly property int barHeight: sz?.barHeight ?? 40
    readonly property int toastWidth: sz?.toastWidth ?? 420
    readonly property int osdWidth: sz?.osdWidth ?? 360
    readonly property int panelWidth: sz?.panelWidth ?? 640
    readonly property int transportCell: sz?.transportCell ?? 48
    readonly property int ditherTile: sz?.ditherTile ?? 4
    readonly property int ditherTileBig: sz?.ditherTileBig ?? 8
    readonly property int barSeam: sz?.barSeam ?? 3
    readonly property int toastGap: sz?.toastGap ?? 22
    readonly property int transportCellSmall: sz?.transportCellSmall ?? 36
    readonly property int transportMain: sz?.transportMain ?? 64
    readonly property int transportMainSmall: sz?.transportMainSmall ?? 48

    // motion
    readonly property int pixelateDuration: tr?.pixelateDuration ?? 500
    readonly property int pixelateSteps: tr?.pixelateSteps ?? 4
    readonly property int pixelateCellStart: tr?.pixelateCellStart ?? 16
    readonly property int pixelateStagger: tr?.pixelateStagger ?? 60
    readonly property int revealSteps: tr?.revealSteps ?? 3
    // Game Mode or reduced motion: every transition becomes a cut.
    readonly property bool cut: tr?.cut ?? false
}
