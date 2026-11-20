pragma ComponentBehavior: Bound
import QtQuick
import "AppleLogic.js" as Logic

// The four Claude cards for Apple things in one place, sharing one Claude
// runner. A panel (or the launcher's `?` hook, or a notification) calls
// show(); the card then waits for its own press before anything is sent.
//
//   AppleCards { id: cards; calendars: Calendars; cider: Cider }
//   cards.route("dinner with kuro sat 8")   -> true when a card took it
//   cards.show("free", "when am I free this week")
//   cards.showNote(calendarEvent)           // before an event
//
// route() returns false for words that are not about the calendar or music,
// so the caller can pass them on to the ask panel.
Item {
    id: root

    property var calendars: null
    property var cider: null
    property string defaultCalendar: ""
    property date now: new Date()
    // "", "event", "free", "note", "queue"
    property string mode: ""
    readonly property alias claude: runner
    readonly property bool open: root.mode !== ""

    signal closed()

    implicitWidth: card.item ? card.item.implicitWidth : 0
    implicitHeight: card.item ? card.item.implicitHeight : 0

    AppleClaude { id: runner }

    function route(text) {
        const kind = Logic.intent(text);
        if (kind === "other")
            return false;
        root.show(kind, text);
        return true;
    }

    function show(kind, text) {
        runner.cancel();
        const same = root.mode === kind;
        card.pendingText = text ?? "";
        root.mode = kind;
        if (same && card.item)
            card.apply();
    }

    function showNote(ev) {
        runner.cancel();
        const same = root.mode === "note";
        card.pendingEvent = ev;
        root.mode = "note";
        if (same && card.item)
            card.apply();
    }

    function close() {
        runner.cancel();
        root.mode = "";
        root.closed();
    }

    Loader {
        id: card
        property string pendingText: ""
        property var pendingEvent: null
        active: root.mode !== ""
        sourceComponent: root.mode === "event" ? eventCard : root.mode === "free" ? freeCard : root.mode === "note" ? noteCard : root.mode === "queue" ? queueCard : null
        onLoaded: apply()

        function apply() {
            if (!item)
                return;
            if (root.mode === "note")
                item.reset(pendingEvent);
            else
                item.reset(pendingText);
            item.forceActiveFocus();
        }
    }

    Component {
        id: eventCard
        AppleEventCard {
            claude: runner
            calendars: root.calendars
            defaultCalendar: root.defaultCalendar
            now: root.now
            onDismissed: root.close()
            onCreated: root.close()
        }
    }
    Component {
        id: freeCard
        AppleFreeTime {
            claude: runner
            calendars: root.calendars
            now: root.now
            onDismissed: root.close()
        }
    }
    Component {
        id: noteCard
        AppleEventNote {
            claude: runner
            now: root.now
            onSnooze: root.close()
            onOpenInvite: root.close()
        }
    }
    Component {
        id: queueCard
        AppleQueueCard {
            claude: runner
            cider: root.cider
            onDismissed: root.close()
        }
    }
}
