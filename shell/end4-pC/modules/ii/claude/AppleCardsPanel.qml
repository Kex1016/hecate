pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.services.apple
import qs.services.claude
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import Quickshell
import Quickshell.Wayland

// Where the Claude cards for Apple things show (AppleWidgets board): the launcher's `?`
// tries them first (a new event, a free-time question, a music request), and the event note
// comes up by itself ten minutes before an event. Every card waits for a press before it
// sends anything, so showing one never asks Claude.
//
// Same corner as the ask panel, which it stands in for: a card the launcher opened takes the
// keyboard until it closes; the note before an event is a toast and only takes it on a click.
Scope {
    id: root

    readonly property bool open: cards.open
    readonly property bool asToast: cards.mode === "note"

    // The launcher's `?` (through ClaudeAsk): true when a card took the words.
    function route(text) {
        cards.now = new Date();
        return cards.route(text);
    }

    Component.onCompleted: ClaudeAsk.cardsHook = text => root.route(text)
    Component.onDestruction: {
        if (ClaudeAsk.cardsHook !== null)
            ClaudeAsk.cardsHook = null;
        ClaudeAsk.cardOpen = false;
    }
    // the fail toast steps aside for a card the launcher opened (a note is a toast itself)
    Binding {
        target: ClaudeAsk
        property: "cardOpen"
        value: root.open && !root.asToast
    }

    // ── the note before an event ───────────────────────────────────────────────

    readonly property int noteLeadMs: 10 * 60 * 1000
    // events already offered, by start and title, so each comes up once
    property var noted: ({})

    function eventKey(ev) {
        return `${ev.start}|${ev.title ?? ""}`;
    }

    function checkUpcoming() {
        if (!Calendars.ready || cards.open || Notifications.silent || GlobalStates.screenLocked)
            return;
        const now = Date.now();
        const next = Calendars.upcoming(5).find(e => !e.allDay
            && Calendars.startMs(e) - now <= root.noteLeadMs
            && !root.noted[root.eventKey(e)]);
        if (!next)
            return;
        const marks = Object.assign({}, root.noted);
        marks[root.eventKey(next)] = true;
        root.noted = marks;
        cards.now = new Date();
        cards.showNote(next);
    }

    Timer {
        running: Calendars.ready
        interval: 30 * 1000
        repeat: true
        triggeredOnStart: true
        onTriggered: root.checkUpcoming()
    }

    // ── the layer ──────────────────────────────────────────────────────────────

    PanelWindow {
        id: panelWindow

        readonly property int pad: fx.margin

        screen: Quickshell.screens.find(s => s.name === WM.focusedMonitor?.name) ?? Quickshell.screens[0]
        // stays mapped while the close frames play
        visible: root.open || fx.present

        WlrLayershell.namespace: "quickshell:claudeCards"
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: !root.open ? WlrKeyboardFocus.None
            : root.asToast ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.Exclusive
        exclusionMode: ExclusionMode.Ignore
        color: "transparent"

        anchors {
            top: true
            right: true
        }
        // where the ask panel sits (50px under the bar, 60px from the right); a note comes
        // in where the toasts do, under the claude toasts
        margins {
            top: (root.asToast ? H.HTokens.barHeight + 32 + SessionsState.toastsHeight : H.HTokens.barHeight + 50) - pad
            right: (root.asToast ? 24 : 60) - pad
        }

        implicitWidth: Math.max(1, fx.implicitWidth) + 2 * pad
        implicitHeight: Math.max(1, fx.implicitHeight) + 2 * pad

        mask: Region {
            item: root.open ? fx : null
        }

        Connections {
            target: cards
            function onOpenChanged() {
                if (cards.open && !root.asToast)
                    GlobalFocusGrab.addDismissable(panelWindow);
                else
                    GlobalFocusGrab.removeDismissable(panelWindow);
            }
        }
        Connections {
            target: GlobalFocusGrab
            function onDismissed() {
                if (cards.open && !root.asToast)
                    cards.close();
            }
        }

        H.PixelTransition {
            id: fx
            x: panelWindow.pad
            y: panelWindow.pad
            shown: root.open

            AppleCards {
                id: cards
                calendars: Calendars
                cider: Cider
                defaultCalendar: Calendars.defaultTarget
            }
        }
    }
}
