pragma Singleton
import QtQuick
import Quickshell

// What the sessions panel shares with the rest of the shell, so the bar can
// reach it without a GlobalStates entry:
//
//   SessionsState.open        the panel is showing (the bar's claude segment toggles it)
//   SessionsState.waiting     sessions waiting for an answer (a bar badge, and what a
//                             fullscreen window hides the toasts behind)
//   SessionsState.toastsHeight  height of the claude toast stack, so the notification
//                             stack can sit below it (both start at the same corner)
Singleton {
    property bool open: false
    property int waiting: 0
    property real toastsHeight: 0
}
