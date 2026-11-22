import QtQuick
import qs.modules.common.widgets.haiiro

// The design system's PixelGlyphs: small pixel pictures that sit beside a
// free widget's content, never in place of it. `name` is "cloud" (20x9),
// "bell" (12x9, gold), "toggleOff" / "toggleOn" (12x7) or "clipboard"
// (11x12). Art pixels are 4px (the bell 5px), with the 2px base drop shadow.
// Use one per widget at most, where it says something the words do not.
//   PixelGlyph { name: "bell" }
PixelArt {
    id: root

    property string name: "cloud"

    readonly property var glyphs: ({
        cloud: [
            "       ####         ",
            "     ########       ",
            "    ##########      ",
            "  ###############   ",
            " ################## ",
            "####################",
            "###################+",
            " #################+ ",
            "  ++++++++++++++++  "
        ],
        bell: [
            "     GG     ",
            "   GGGGGG   ",
            "  GGGGGGGG  ",
            "  GGGGGGGG  ",
            "  GGGGGGGG  ",
            "  GGGGGGGG  ",
            " GGGGGGGGGG ",
            "GGGGGGGGGGGG",
            "     GG     "
        ],
        toggleOff: [
            " hhhhhhhhhh ",
            "hh........hh",
            "h.+++......h",
            "h.+++......h",
            "h.+++......h",
            "hh........hh",
            " hhhhhhhhhh "
        ],
        toggleOn: [
            " FFFFFFFFFF ",
            "FFPPPPPPPPFF",
            "FPPPPPP###PF",
            "FPPPPPP###PF",
            "FPPPPPP###PF",
            "FFPPPPPPPPFF",
            " FFFFFFFFFF "
        ],
        clipboard: [
            "   +++++   ",
            "###+###+###",
            "#.........#",
            "#.++++++..#",
            "#.........#",
            "#.++++....#",
            "#.........#",
            "#.+++++++.#",
            "#.........#",
            "#.+++.....#",
            "#.........#",
            "###########"
        ]
    })

    cell: name === "bell" ? 5 : 4
    rows: glyphs[name] ?? []
}
