# haiiro kit

QML parts of the haiiro design system, one file per part. Every screen of
the shell is built from these; read the design system's component notes for
when to use which.

```qml
import qs.modules.common.widgets.haiiro as H

H.PixelTransition {
    shown: panel.open
    H.HWindow {
        label: "hecate: ~/launch"
        hints: [{ key: "enter", text: "open" }, { key: "esc", text: "close" }]
        H.PromptLine { input: true; role: "nav" }
    }
}
```

Import it with a qualifier (`as H`) in files that also import
QtQuick.Controls, whose `Slider` has the same name.

| file | part |
| --- | --- |
| HWindow | Window: pine, love, gold and foam frames, a coloured label (`labelInk`), footer hints and note |
| InkButton | InkButton (c1-c7, flat) |
| Chip | Chip and badge; `hoverable` chips that aren't toggles, `pressAndHold()` (also right click) |
| Keycap | Keycap, with an optional hint after it |
| ListRow | ListRow, with match highlight and leading/trailing slots |
| Slider | Slider (and the OSD's bare track); emits `moved` and never writes `value` |
| Segmented, Spin | Segmented and its number spinner |
| SettingRow | SettingRow |
| Progress | Progress, with an `ink` (love for a disk that is nearly full) |
| Transport | Transport (normal and small) |
| CalendarGrid | Calendar |
| Spark | Sparkline and the hourly bars |
| Dither, Veil | Dither placeholders and the dimming veil |
| CodeBlock | CodeBlock |
| PromptLine | PromptLine, and the search field form |
| PasswordLine | the sudo/polkit password prompt: words, optional stars, block cursor |
| PixelTransition | the open and close effect for every panel |
| HText, HLabel, HRule | text in a type role, the `.lab` heading, the `.hr` rule |
| HTokens | where the kit reads tokens from `Appearance.haiiro` |
| FreeWidget | the root of a free widget: prompt-line label (`handle`), `art` slot, `live`, the clearing |
| FreeText | HText with the hard base shadow; `size` for figures outside the type roles |
| FreeArt | album art or a portrait on the 8px hard shadow |
| PixelGrid | the base of every pixel shape: `px()` and an origin on whole device pixels |
| PixelArt | a picture from rows of characters, one per art pixel; `shadow`, `dim`, `mirror` |
| PixelCells | one square per unit; wraps to a width; `lit` for stepping squares |
| DayRuler | the day as 24 ticks |
| RainColumns | rain chance per hour under a dithered cloud line |
| MirrorSpark | two series on one baseline |
| PixelRing | 60 squares around a value |
| PixelBattery | the battery beside its percentage |
| PixelMoon, SunPath | the moon at its phase, the sun on its path |
| PixelBuds | AirPods: bud, case, Max, and the dim set |
| PixelGlyph | PixelGlyphs: cloud, bell, toggle off and on, clipboard |
| GlyphTransport, PlayLine | bare transport glyphs on 44px targets, the dotted playhead line |
| RuledNote | lines over dashed hairlines, editable in place |
| PixelCheck | the square checkbox of the vim todo |
| TextBar | htop's bar in text |
| BlockCursor | the block cursor that ends a line |
| PixelChecker | a 2x2 checker over what is behind; a dashed hairline at one pixel high |
| Gallery | every part on one sheet, to compare with the design system |

Rules the kit keeps so screens do not have to: square corners, no easing,
no fades, no blur, whole-pixel line boxes for the bitmap face, the foam focus
outline, and `Appearance.haiiro.transition.cut` turning every transition into
a cut.

## Free widgets

A free widget is a `FreeWidget` with `FreeText` and pixel shapes inside:

```qml
H.FreeWidget {
    label: "df -h"
    live: visible
    H.PixelCells { count: 25; filled: 11; large: true }
    H.FreeText { text: "one square is 4%"; role: "caption"; ink: "subtle" }
}
```

- The clearing is drawn outside the widget's bounds and is no part of its
  size. It lines up on one checker grid for the whole scene, so two that
  overlap do not add up. Set `clearingLayer` to an item under every widget to
  keep one widget's clearing from covering its neighbour's content.
- Every pixel shape is a `PixelGrid`: lengths go through `px()`, which rounds
  them to whole device pixels (an 8px cell is 10 at scale 1.25, its 2px gap
  3), and the shape's origin is nudged onto a device pixel wherever a layout
  put it. Widget files draw no rectangles of their own; a new shape belongs
  here, built on `PixelGrid` or `PixelArt`.
- Text takes `shadow: "free"` (what `FreeText` sets) or `"title"` for the
  display face. Art takes `FreeArt`.
- The clearing and the checker are shaders (`shaders/*.frag`, with the built
  `.qsb` beside them: `qsb --qt6 -o x.frag.qsb x.frag`). They need the GL or
  Vulkan scene graph; the software renderer skips them.

