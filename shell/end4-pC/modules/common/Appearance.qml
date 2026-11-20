pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell

/**
 * haiiro is the theme. One static set of tokens, named after the design system's tokens.json
 * (color-highlight-med is haiiro.color.highlightMed, pixelate-duration is
 * haiiro.transition.pixelateDuration). Nothing here is read from the wallpaper or from a setting.
 *
 * New code reads Appearance.haiiro.*. The groups after it (m3colors, colors, rounding, font,
 * animationCurves, animation, sizes) are the names the modules written for Material You still
 * read. Each of them resolves to a token, and they go when the last of those modules does.
 */
Singleton {
    id: root

    // Game Mode. Whoever owns the toggle sets this; with reduced motion it turns every
    // transition into a cut (haiiro.transition.cut).
    property bool gameMode: false

    readonly property QtObject haiiro: QtObject {
        // Rosé Pine (main).
        readonly property QtObject color: QtObject {
            id: hc
            readonly property color base: "#191724"
            readonly property color surface: "#1f1d2e"
            readonly property color overlay: "#26233a"
            readonly property color muted: "#6e6a86"
            readonly property color subtle: "#908caa"
            readonly property color text: "#e0def4"
            readonly property color love: "#eb6f92"
            readonly property color gold: "#f6c177"
            readonly property color rose: "#ebbcba"
            readonly property color pine: "#31748f"
            readonly property color foam: "#9ccfd8"
            readonly property color iris: "#c4a7e7"
            readonly property color highlightLow: "#21202e"
            readonly property color highlightMed: "#403d52"
            readonly property color highlightHigh: "#524f67"
            // color-text at 55%, 25% and 14%.
            readonly property color inkMuted: Qt.rgba(hc.text.r, hc.text.g, hc.text.b, 0.55)
            readonly property color inkHairline: Qt.rgba(hc.text.r, hc.text.g, hc.text.b, 0.25)
            readonly property color inkTrack: Qt.rgba(hc.text.r, hc.text.g, hc.text.b, 0.14)
            // color-base at 55%, under the dither veil of a full-screen panel.
            readonly property color veilHeavy: Qt.rgba(hc.base.r, hc.base.g, hc.base.b, 0.55)
        }

        // The bundled faces (assets/fonts/haiiro). Set font.family to mono or display. Bold is
        // the 2x VGA face, registered as the mono family's weight 700, so font.bold picks it.
        // QML takes one family per font; the stacks are the order glyphs fall back in, which
        // fontconfig applies (assets/fonts/haiiro/45-haiiro.conf).
        readonly property QtObject font: QtObject {
            id: hf
            readonly property string mono: "IBM VGA 8x16"
            readonly property string display: "Jacquard 24"
            readonly property string fallback: "Unifont"
            readonly property string fallbackUpper: "Unifont Upper"
            readonly property list<string> monoStack: [hf.mono, hf.fallback, hf.fallbackUpper]
            readonly property list<string> displayStack: [hf.display, hf.mono, hf.fallback]
            readonly property real lineHeight: 1.15
        }

        // Type styles, in pixels. body is the VGA face's native size: keep it at exactly 16.
        // body-bold is body with font.bold.
        readonly property QtObject fontSize: QtObject {
            readonly property int displayHuge: 150
            readonly property int nav: 32
            readonly property int valueLg: 20
            readonly property int body: 16
            readonly property int headingSm: 14
            readonly property int small: 13
            readonly property int label: 12
            readonly property int badge: 11
        }

        // The uppercase styles' tracking (0.08em, 0.08em, 0.06em), in pixels for font.letterSpacing.
        readonly property QtObject letterSpacing: QtObject {
            readonly property real headingSm: 1.12
            readonly property real label: 0.96
            readonly property real badge: 0.66
        }

        // space-1 … space-20.
        readonly property QtObject space: QtObject {
            readonly property int s1: 4
            readonly property int s2: 8
            readonly property int s3: 12
            readonly property int s4: 16
            readonly property int s6: 24
            readonly property int s8: 32
            readonly property int s12: 50
            readonly property int s20: 80
        }

        readonly property QtObject border: QtObject {
            readonly property int hairline: 1
            readonly property int divider: 3
            // The window frame: always double, in pine.
            readonly property int window: 5
        }

        // haiiro is square.
        readonly property QtObject radius: QtObject {
            readonly property int none: 0
        }

        readonly property QtObject shadow: QtObject {
            // The hard offset under a display title.
            readonly property QtObject title: QtObject {
                readonly property int x: 8
                readonly property int y: 8
                readonly property color color: hc.base
            }
        }

        // Fixed sizes. The shell is on rails: these are not settings.
        readonly property QtObject size: QtObject {
            readonly property int barHeight: 40
            // The overlay line under the bar.
            readonly property int barSeam: 3
            readonly property int toastWidth: 420
            // Between stacked toasts, so their window labels clear each other.
            readonly property int toastGap: 22
            readonly property int osdWidth: 360
            readonly property int panelWidth: 640
            // The quick panel (Mod+N).
            readonly property int quickPanelWidth: 480
            readonly property int transportCell: 48
            readonly property int transportCellSmall: 36
            // Play/pause.
            readonly property int transportMain: 64
            readonly property int transportMainSmall: 48
            readonly property int ditherTile: 4
            readonly property int ditherTileBig: 8
        }

        // The live background. The compositor draws it; these are the same numbers.
        readonly property QtObject blob: QtObject {
            // One simulation cell, in pixels.
            readonly property int cell: 6
            // Target count = cells / density, clamped to countMin..countMax.
            readonly property int density: 700
            readonly property int countMin: 6
            readonly property int countMax: 60
            // In cells.
            readonly property int radiusMax: 24
            // In cells per second.
            readonly property real speedMin: 1.5
            readonly property real speedMax: 6
            readonly property int fps: 30
            readonly property color ground: hc.base
            readonly property color halo: hc.overlay
            readonly property color core: hc.highlightMed
        }

        // Things pixelate in stepped frames; nothing slides or eases.
        readonly property QtObject transition: QtObject {
            // Milliseconds.
            readonly property int pixelateDuration: 500
            // Frames in a pixelate: cells of pixelateCellStart, halved each step until sharp.
            readonly property int pixelateSteps: 4
            readonly property int pixelateCellStart: 16
            // Milliseconds between items that pixelate in as a group.
            readonly property int pixelateStagger: 60
            readonly property int revealSteps: 3
            readonly property string workspaceSwitch: "cut"
            readonly property string borderColourChange: "snap"
            // Game Mode or reduced motion: every transition is a cut.
            readonly property bool cut: root.gameMode || (Config.options?.motion?.reduced ?? false)
        }
    }

    Instantiator {
        model: ["IBMVGA8x16-Regular.ttf", "IBMVGA8x16-Bold.ttf", "Jacquard24-Regular.ttf", "Unifont.otf", "UnifontUpper.otf"]
        delegate: FontLoader {
            required property string modelData
            source: Qt.resolvedUrl(`${Quickshell.shellPath("assets/fonts/haiiro")}/${modelData}`)
            onStatusChanged: {
                if (status === FontLoader.Error)
                    console.warn(`[Appearance] could not load assets/fonts/haiiro/${modelData}`);
            }
        }
    }

    // Everything below is for modules that still read the Material You names.

    function getColorFromName(name) {
        switch (name) {
            case "primary":            return colors.colPrimary
            case "secondary":          return colors.colSecondary
            case "tertiary":           return colors.colTertiary
            case "primaryContainer":   return colors.colPrimaryContainer
            case "secondaryContainer": return colors.colSecondaryContainer
            case "tertiaryContainer":  return colors.colTertiaryContainer
            case "layer0":             return colors.colLayer0
            case "layer1":             return colors.colLayer1
            case "layer0Border":       return colors.colLayer0Border
            case "black":              return hc.base
            default:                   return colors.colPrimaryContainer
        }
    }

    readonly property string syntaxHighlightingTheme: "Monokai"

    readonly property QtObject m3colors: QtObject {
        readonly property bool darkmode: true
        readonly property bool transparent: false
        readonly property color m3background: hc.base
        readonly property color m3onBackground: hc.text
        readonly property color m3surface: hc.surface
        readonly property color m3surfaceDim: hc.base
        readonly property color m3surfaceBright: hc.highlightHigh
        readonly property color m3surfaceContainerLowest: hc.base
        readonly property color m3surfaceContainerLow: hc.surface
        readonly property color m3surfaceContainer: hc.overlay
        readonly property color m3surfaceContainerHigh: hc.highlightMed
        readonly property color m3surfaceContainerHighest: hc.highlightHigh
        readonly property color m3onSurface: hc.text
        readonly property color m3surfaceVariant: hc.overlay
        readonly property color m3onSurfaceVariant: hc.subtle
        readonly property color m3inverseSurface: hc.text
        readonly property color m3inverseOnSurface: hc.surface
        readonly property color m3outline: hc.muted
        readonly property color m3outlineVariant: hc.highlightMed
        readonly property color m3shadow: hc.base
        readonly property color m3scrim: hc.base
        readonly property color m3surfaceTint: hc.pine
        readonly property color m3primary: hc.pine
        readonly property color m3onPrimary: hc.text
        readonly property color m3primaryContainer: hc.highlightMed
        readonly property color m3onPrimaryContainer: hc.text
        readonly property color m3inversePrimary: hc.pine
        readonly property color m3secondary: hc.foam
        readonly property color m3onSecondary: hc.surface
        readonly property color m3secondaryContainer: hc.highlightMed
        readonly property color m3onSecondaryContainer: hc.text
        readonly property color m3tertiary: hc.iris
        readonly property color m3onTertiary: hc.surface
        readonly property color m3tertiaryContainer: hc.highlightMed
        readonly property color m3onTertiaryContainer: hc.text
        readonly property color m3error: hc.love
        readonly property color m3onError: hc.surface
        readonly property color m3errorContainer: hc.love
        readonly property color m3onErrorContainer: hc.surface
        readonly property color m3primaryFixed: hc.pine
        readonly property color m3primaryFixedDim: hc.pine
        readonly property color m3onPrimaryFixed: hc.text
        readonly property color m3onPrimaryFixedVariant: hc.text
        readonly property color m3secondaryFixed: hc.foam
        readonly property color m3secondaryFixedDim: hc.foam
        readonly property color m3onSecondaryFixed: hc.surface
        readonly property color m3onSecondaryFixedVariant: hc.surface
        readonly property color m3tertiaryFixed: hc.iris
        readonly property color m3tertiaryFixedDim: hc.iris
        readonly property color m3onTertiaryFixed: hc.surface
        readonly property color m3onTertiaryFixedVariant: hc.surface
        readonly property color m3success: hc.foam
        readonly property color m3onSuccess: hc.surface
        readonly property color m3successContainer: hc.highlightMed
        readonly property color m3onSuccessContainer: hc.foam
    }

    // The window fill is surface; each layer above it is the next tone up.
    readonly property QtObject colors: QtObject {
        readonly property color colSubtext: hc.subtle
        // Layer 0
        readonly property color colLayer0Base: hc.surface
        readonly property color colLayer0: hc.surface
        readonly property color colUiBackground: hc.surface
        readonly property color colUiPopupBackground: hc.overlay
        readonly property color colUiSurfaceContainer: hc.overlay
        readonly property color colOnLayer0: hc.text
        readonly property color colLayer0Hover: hc.overlay
        readonly property color colLayer0Active: hc.highlightMed
        readonly property color colLayer0Border: hc.pine
        // Layer 1
        readonly property color colLayer1Base: hc.overlay
        readonly property color colLayer1: hc.overlay
        readonly property color colOnLayer1: hc.text
        readonly property color colOnLayer1Inactive: hc.muted
        readonly property color colLayer1Hover: hc.highlightMed
        readonly property color colLayer1Active: hc.highlightHigh
        // Layer 2
        readonly property color colLayer2Base: hc.highlightMed
        readonly property color colLayer2: hc.highlightMed
        readonly property color colLayer2Hover: hc.highlightHigh
        readonly property color colLayer2Active: hc.muted
        readonly property color colLayer2Disabled: hc.overlay
        readonly property color colOnLayer2: hc.text
        readonly property color colOnLayer2Disabled: hc.muted
        // Layer 3
        readonly property color colLayer3Base: hc.highlightHigh
        readonly property color colLayer3: hc.highlightHigh
        readonly property color colLayer3Hover: hc.muted
        readonly property color colLayer3Active: hc.muted
        readonly property color colOnLayer3: hc.text
        // Layer 4
        readonly property color colLayer4Base: hc.highlightHigh
        readonly property color colLayer4: hc.highlightHigh
        readonly property color colLayer4Hover: hc.muted
        readonly property color colLayer4Active: hc.muted
        readonly property color colOnLayer4: hc.text
        // Primary: pine, the current thing
        readonly property color colPrimary: hc.pine
        readonly property color colOnPrimary: hc.text
        readonly property color colPrimaryHover: hc.pine
        readonly property color colPrimaryActive: hc.pine
        readonly property color colPrimaryContainer: hc.highlightMed
        readonly property color colPrimaryContainerHover: hc.highlightHigh
        readonly property color colPrimaryContainerActive: hc.highlightHigh
        readonly property color colOnPrimaryContainer: hc.text
        // Secondary: foam, values and paths
        readonly property color colSecondary: hc.foam
        readonly property color colSecondaryHover: hc.foam
        readonly property color colSecondaryActive: hc.foam
        readonly property color colOnSecondary: hc.surface
        readonly property color colSecondaryContainer: hc.highlightMed
        readonly property color colSecondaryContainerHover: hc.highlightHigh
        readonly property color colSecondaryContainerActive: hc.highlightHigh
        readonly property color colOnSecondaryContainer: hc.text
        // Tertiary: iris, media and files
        readonly property color colTertiary: hc.iris
        readonly property color colTertiaryHover: hc.iris
        readonly property color colTertiaryActive: hc.iris
        readonly property color colTertiaryContainer: hc.highlightMed
        readonly property color colTertiaryContainerHover: hc.highlightHigh
        readonly property color colTertiaryContainerActive: hc.highlightHigh
        readonly property color colOnTertiary: hc.surface
        readonly property color colOnTertiaryContainer: hc.text
        // Surface
        readonly property color colBackgroundSurfaceContainer: hc.overlay
        readonly property color colSurfaceContainerLow: hc.surface
        readonly property color colSurfaceContainer: hc.overlay
        readonly property color colSurfaceContainerHigh: hc.highlightMed
        readonly property color colSurfaceContainerHighest: hc.highlightHigh
        readonly property color colSurfaceContainerHighestHover: hc.muted
        readonly property color colSurfaceContainerHighestActive: hc.muted
        readonly property color colSurfaceVariant: hc.overlay
        readonly property color colOnSurface: hc.text
        readonly property color colOnSurfaceVariant: hc.subtle
        // Misc
        readonly property color colTooltip: hc.overlay
        readonly property color colOnTooltip: hc.text
        readonly property color colScrim: hc.veilHeavy
        readonly property color colShadow: hc.base
        readonly property color colOutline: hc.muted
        readonly property color colOutlineVariant: hc.highlightMed
        // Error: love, critical or failed
        readonly property color colError: hc.love
        readonly property color colErrorHover: hc.love
        readonly property color colErrorActive: hc.love
        readonly property color colOnError: hc.surface
        readonly property color colErrorContainer: hc.love
        readonly property color colErrorContainerHover: hc.love
        readonly property color colErrorContainerActive: hc.love
        readonly property color colOnErrorContainer: hc.surface
    }

    // Square.
    readonly property QtObject rounding: QtObject {
        readonly property int unsharpen: 0
        readonly property int unsharpenmore: 0
        readonly property int verysmall: 0
        readonly property int small: 0
        readonly property int normal: 0
        readonly property int large: 0
        readonly property int verylarge: 0
        readonly property int full: 0
        readonly property int screenRounding: 0
        readonly property int windowRounding: 0
    }

    // One face. Icons keep their own fonts.
    readonly property QtObject font: QtObject {
        readonly property QtObject family: QtObject {
            readonly property string main: hf.mono
            readonly property string numbers: hf.mono
            readonly property string title: hf.mono
            readonly property string iconMaterial: "Material Symbols Lite"
            readonly property string iconNerd: "JetBrains Mono NF"
            readonly property string monospace: hf.mono
            readonly property string reading: hf.mono
            readonly property string expressive: hf.mono
        }
        // A bitmap face has no axes.
        readonly property QtObject variableAxes: QtObject {
            readonly property var main: ({})
            readonly property var numbers: ({})
            readonly property var title: ({})
        }
        // The old ten sizes, on the type scale: badge, label, small, body and value-lg.
        readonly property QtObject pixelSize: QtObject {
            readonly property int smallest: 11
            readonly property int smaller: 12
            readonly property int smallie: 13
            readonly property int small: 16
            readonly property int normal: 16
            readonly property int large: 16
            readonly property int larger: 20
            readonly property int huge: 20
            readonly property int hugeass: 20
            readonly property int title: 20
        }
    }

    // Nothing eases: every duration is 0, so an animation built from these is a cut. The
    // curves stay valid for code that names one beside a duration of its own.
    readonly property QtObject animationCurves: QtObject {
        readonly property list<real> expressiveFastSpatial: [0.42, 1.67, 0.21, 0.90, 1, 1]
        readonly property list<real> expressiveDefaultSpatial: [0.38, 1.21, 0.22, 1.00, 1, 1]
        readonly property list<real> expressiveSlowSpatial: [0.39, 1.29, 0.35, 0.98, 1, 1]
        readonly property list<real> expressiveEffects: [0.34, 0.80, 0.34, 1.00, 1, 1]
        readonly property list<real> emphasized: [0.05, 0, 2 / 15, 0.06, 1 / 6, 0.4, 5 / 24, 0.82, 0.25, 1, 1, 1]
        readonly property list<real> emphasizedFirstHalf: [0.05, 0, 2 / 15, 0.06, 1 / 6, 0.4, 5 / 24, 0.82]
        readonly property list<real> emphasizedLastHalf: [5 / 24, 0.82, 0.25, 1, 1, 1]
        readonly property list<real> emphasizedAccel: [0.3, 0, 0.8, 0.15, 1, 1]
        readonly property list<real> emphasizedDecel: [0.05, 0.7, 0.1, 1, 1, 1]
        readonly property list<real> standard: [0.2, 0, 0, 1, 1, 1]
        readonly property list<real> standardAccel: [0.3, 0, 1, 1, 1, 1]
        readonly property list<real> standardDecel: [0, 0, 0, 1, 1, 1]
        readonly property real expressiveFastSpatialDuration: 0
        readonly property real expressiveDefaultSpatialDuration: 0
        readonly property real expressiveSlowSpatialDuration: 0
        readonly property real expressiveEffectsDuration: 0
    }

    component Cut: QtObject {
        readonly property int duration: 0
        readonly property int type: Easing.Linear
        readonly property list<real> bezierCurve: [0, 0, 1, 1, 1, 1]
        readonly property int velocity: 650
        readonly property Component numberAnimation: Component {
            NumberAnimation {
                duration: 0
            }
        }
        readonly property Component colorAnimation: Component {
            ColorAnimation {
                duration: 0
            }
        }
    }

    readonly property QtObject animation: QtObject {
        readonly property QtObject elementMove: Cut {}
        readonly property QtObject elementMoveSmall: Cut {}
        readonly property QtObject elementMoveEnter: Cut {}
        readonly property QtObject elementMoveExit: Cut {}
        readonly property QtObject elementMoveFast: Cut {}
        readonly property QtObject elementResize: Cut {}
        readonly property QtObject clickBounce: Cut {}
        readonly property QtObject scroll: Cut {}
        readonly property QtObject menuDecel: Cut {}
        readonly property QtObject sidebarSlideEnter: Cut {}
        readonly property QtObject sidebarSlideExit: Cut {}
    }

    readonly property QtObject sizes: QtObject {
        readonly property real baseBarHeight: root.haiiro.size.barHeight
        readonly property real barHeight: root.haiiro.size.barHeight
        readonly property real barCenterSideModuleWidth: 360
        readonly property real barCenterSideModuleWidthShortened: 280
        readonly property real barCenterSideModuleWidthHellaShortened: 190
        readonly property real barShortenScreenWidthThreshold: 1200
        readonly property real barHellaShortenScreenWidthThreshold: 1000
        readonly property real elevationMargin: 10
        readonly property real fabShadowRadius: 5
        readonly property real fabHoveredShadowRadius: 7
        readonly property real hyprlandGapsOut: 5
        readonly property real mediaControlsWidth: 440
        readonly property real mediaControlsHeight: 160
        readonly property real notificationPopupWidth: root.haiiro.size.toastWidth
        readonly property real osdWidth: root.haiiro.size.osdWidth
        readonly property real searchWidthCollapsed: 210
        readonly property real searchWidth: 360
        readonly property real sidebarWidth: 460
        readonly property real sidebarWidthExtended: 750
        readonly property real baseVerticalBarWidth: 46
        readonly property real verticalBarWidth: 46
        readonly property real wallpaperSelectorWidth: 1200
        readonly property real wallpaperSelectorHeight: 690
        readonly property real wallpaperSelectorItemMargins: 8
        readonly property real wallpaperSelectorItemPadding: 6
    }
}
