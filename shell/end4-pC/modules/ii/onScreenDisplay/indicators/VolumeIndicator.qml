import qs.services
import QtQuick
import qs.modules.ii.onScreenDisplay

OsdValueIndicator {
    readonly property bool isMuted: Audio.sink?.audio.muted ?? false
    name: "vol"
    value: Audio.sink?.audio.volume ?? 0
    variant: isMuted ? "love" : "pine"
    message: isMuted ? "muted" : ""
}
