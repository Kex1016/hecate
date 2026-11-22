import QtQuick
import qs.modules.common.widgets.haiiro

// AirPods on a pixel grid. `part` picks the picture: "left" or "right" (a
// bud, 11x16), "case" (25x13, the lid line in subtle, the status light in
// foam) or "max" (AirPods Max, 24x15). One art pixel is 6px (5px for Max);
// set `cell: 4` for the small set shown while not connected. `dim` is a bud
// out of your ear, or any part while not connected. The art never changes
// colour for a low battery: the charge under it does.
//   PixelBuds { part: "left" }   PixelBuds { part: "case" }   PixelBuds { part: "right"; dim: true }
PixelArt {
    id: root

    property string part: "left"

    readonly property var bud: [
        "  #####    ",
        " #######hh ",
        "########hhh",
        "#..#####hhh",
        "########hh ",
        "#######++  ",
        " #####++   ",
        " ###+      ",
        " ###+      ",
        " ###+      ",
        " ###+      ",
        " ###+      ",
        " ###+      ",
        " ###+      ",
        " ###+      ",
        " mmm+      "
    ]
    readonly property var caseArt: [
        "  ####################+  ",
        " ######################+ ",
        "########################+",
        "########################+",
        "+++++++++++++++++++++++++",
        "########################+",
        "########################+",
        "###########FF###########+",
        "########################+",
        "########################+",
        "########################+",
        " ######################+ ",
        "  +++++++++++++++++++++  "
    ]
    readonly property var maxArt: [
        "      ############      ",
        "    ###++++++++++++###  ",
        "   ##+              +## ",
        "  ##+                +##",
        "  #+                  +#",
        " ##+                  +#",
        " #+                    +",
        "####                  ##",
        "#####                ###",
        "##..#                #..",
        "##..#                #..",
        "##..#                #..",
        "##..#                #..",
        "#####                ###",
        " ###+                +##"
    ]

    cell: part === "max" ? 5 : 6
    mirror: part === "right"
    rows: part === "case" ? caseArt : part === "max" ? maxArt : bud
}
