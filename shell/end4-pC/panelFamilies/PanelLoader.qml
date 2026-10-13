import QtQuick
import Quickshell

import qs.modules.common

// Panels load in the background, between frames (activeAsync), in the order the family lists
// them: `active` built every panel in one go on the UI thread, so nothing showed until the last
// one (settings, overview, ...) was ready.
LazyLoader {
    property bool extraCondition: true
    activeAsync: Config.ready && extraCondition
}
