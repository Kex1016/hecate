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
| HWindow | Window: pine, love, gold and foam frames, footer hints and note |
| InkButton | InkButton (c1-c7, flat) |
| Chip | Chip and badge |
| Keycap | Keycap, with an optional hint after it |
| ListRow | ListRow, with match highlight and leading/trailing slots |
| Slider | Slider (and the OSD's bare track) |
| Segmented, Spin | Segmented and its number spinner |
| SettingRow | SettingRow |
| Progress | Progress |
| Transport | Transport (normal and small) |
| CalendarGrid | Calendar |
| Spark | Sparkline and the hourly bars |
| Dither, Veil | Dither placeholders and the dimming veil |
| CodeBlock | CodeBlock |
| PromptLine | PromptLine, and the search field form |
| PixelTransition | the open and close effect for every panel |
| HText, HLabel, HRule | text in a type role, the `.lab` heading, the `.hr` rule |
| HTokens | where the kit reads tokens from `Appearance.haiiro` |
| Gallery | every part on one sheet, to compare with the design system |

Rules the kit keeps so screens do not have to: square corners, no easing,
no fades, no blur, whole-pixel line boxes for the bitmap face, the foam focus
outline, and `Appearance.haiiro.transition.cut` turning every transition into
a cut.
