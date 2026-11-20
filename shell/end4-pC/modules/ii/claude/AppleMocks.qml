import QtQuick

// Stand-ins for the Calendars and Cider services, for the headless render and
// for trying the cards without an iCloud account. Same shapes as the real
// ones: Calendars { events, targets, defaultTarget, createEvent(ev, done) },
// Cider { library, playQueue(tracks), savePlaylist(name, tracks) }.
QtObject {
    id: root

    property var created: []
    property var played: []
    property var saved: null
    property bool calendarFails: false

    readonly property var calendars: QtObject {
        readonly property var targets: ["local", "iCloud: home", "iCloud: work", "iCloud: shared: friends"]
        readonly property string defaultTarget: "iCloud: home"
        property var events: [
            { title: "server night", start: "2026-11-21T20:00:00", end: "2026-11-21T23:00:00", allDay: false, account: "iCloud", calendar: "shared: friends", notes: "Bring the spare switch. Agenda: NAS move first.", attachments: [{ name: "agenda.md", text: "1. NAS move\n2. Switch the VLANs\n3. Pizza" }] },
            { title: "standup", start: "2026-11-23T10:00:00", end: "2026-11-23T10:30:00", allDay: false, account: "iCloud", calendar: "work", notes: "" },
            { title: "dentist", start: "2026-11-24T14:00:00", end: "2026-11-24T15:00:00", allDay: false, account: "iCloud", calendar: "home", notes: "" },
            { title: "lake trip", start: "2026-11-27T00:00:00", end: "2026-11-29T00:00:00", allDay: true, account: "iCloud", calendar: "home", notes: "" }
        ]
        function createEvent(ev, done) {
            root.created = root.created.concat([ev]);
            done(root.calendarFails ? "The calendar would not take it." : "", ev);
        }
    }

    readonly property var cider: QtObject {
        property var library: [
            { id: "a1", title: "Green", artist: "Hiroshi Yoshimura", album: "Green", seconds: 340 },
            { id: "a2", title: "Through the Looking Glass", artist: "Midori Takada", album: "Through the Looking Glass", seconds: 1380 },
            { id: "a3", title: "Rain", artist: "Hiroshi Yoshimura", album: "Music for Nine Post Cards", seconds: 280 },
            { id: "a4", title: "Mbira", artist: "Midori Takada", album: "Through the Looking Glass", seconds: 420 },
            { id: "a5", title: "Loud Song", artist: "Some Band", album: "Noise", seconds: 200 }
        ]
        function playQueue(tracks) { root.played = tracks; }
        function savePlaylist(name, tracks) { root.saved = { name, tracks }; }
    }
}
