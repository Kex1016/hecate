import qs.services
import QtQuick
import qs.modules.ii.onScreenDisplay

OsdValueIndicator {
    readonly property bool isMuted: Audio.source?.audio.muted ?? false
    name: "mic"
    value: Audio.source?.audio.volume ?? 0
    variant: isMuted ? "love" : "pine"
    message: isMuted ? "muted" : ""
}
