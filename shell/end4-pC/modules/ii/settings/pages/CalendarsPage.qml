pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common
import qs.modules.ii.settings
import qs.modules.common.widgets.haiiro as H

// Calendar accounts: iCloud over CalDAV, any CalDAV server, and .ics
// subscriptions; which calendars show, and where new events go.
SettingsPage {
    id: page

    note: "accounts in ~/.config/hecate/calendars.toml · secrets in the keyring"
    readonly property var hints: [{ key: "enter", text: "edit account" }, { key: "space", text: "show/hide" }, { key: "r", text: "sync now" }]

    // the form under the buttons: "" (none), "icloud", "caldav", "ics"
    property string adding: ""
    property string editingId: ""
    property string error: ""
    property bool busy: false

    function startAdding(kind, id) {
        page.adding = kind;
        page.editingId = id ?? "";
        page.error = "";
        const a = id ? CalendarAccounts.account(id) : null;
        fieldUser.reset(a?.user ?? "");
        fieldUrl.reset(a?.url ?? "");
        fieldName.reset(a?.name ?? "");
        fieldPassword.reset("");
        Qt.callLater(() => (kind === "ics" ? fieldName : kind === "caldav" ? fieldUrl : fieldUser).takeFocus());
    }
    function cancel() {
        page.adding = "";
        page.editingId = "";
        page.error = "";
        fieldPassword.reset("");
    }
    function submit() {
        page.busy = true;
        page.error = "";
        const done = error => {
            page.busy = false;
            page.error = error;
            fieldPassword.reset("");
            if (error === "") page.cancel();
        };
        if (page.adding === "ics") {
            CalendarAccounts.addIcs(fieldName.text, fieldUrl.text, done);
        } else {
            const url = page.adding === "icloud" ? CalendarAccounts.icloudUrl : fieldUrl.text.trim();
            CalendarAccounts.addCalDav(page.adding, url, fieldUser.text.trim(), fieldPassword.text, done, page.editingId || undefined);
        }
    }

    function when(iso) {
        if (!iso) return "";
        const d = new Date(iso);
        const today = new Date().toDateString() === d.toDateString();
        return Qt.locale("en_GB").toString(d, today ? "hh:mm" : "d MMM").toLowerCase();
    }
    function statusLine(a) {
        const s = CalendarAccounts.status[a.id] ?? {};
        if (s.state === "checking") return { text: "checking…", ink: "muted" };
        if (s.state === "error") return { text: "failed", ink: "love" };
        if (a.kind === "ics") return { text: a.refresh >= 1440 ? "daily" : `every ${a.refresh}m`, ink: "muted" };
        if (s.synced) return { text: "synced " + page.when(s.synced), ink: "foam" };
        if (s.checked) return { text: "signed in " + page.when(s.checked), ink: "foam" };
        return { text: "", ink: "muted" };
    }
    function kindInk(kind) {
        return kind === "icloud" ? "gold" : kind === "ics" ? "pine" : "iris";
    }
    // The haiiro ink nearest a calendar's own colour, for its ■.
    function calendarInk(hex) {
        if (!/^#[0-9a-f]{6}/i.test(hex ?? "")) return "foam";
        const inks = ["love", "gold", "rose", "pine", "foam", "iris"];
        const rgb = h => [1, 3, 5].map(i => parseInt(h.slice(i, i + 2), 16));
        const want = rgb(hex);
        let best = "foam", bestD = Infinity;
        for (const name of inks) {
            const c = H.HTokens.ink(name);
            const have = [c.r * 255, c.g * 255, c.b * 255];
            const d = have.reduce((sum, v, i) => sum + (v - want[i]) ** 2, 0);
            if (d < bestD) {
                bestD = d;
                best = name;
            }
        }
        return best;
    }
    function description(a) {
        const s = CalendarAccounts.status[a.id] ?? {};
        if (s.state === "error") return s.message;
        if (a.kind === "icloud") return "CalDAV at caldav.icloud.com. Signs in with an app-specific password from your Apple account page; it lives in the keyring, never in the toml.";
        if (a.kind === "caldav") return `CalDAV at ${CalendarAccounts.hostName(a.url)}. The password lives in the keyring.`;
        return "A read-only subscription URL.";
    }

    Keys.onPressed: event => {
        if (event.key === Qt.Key_R && !(event.modifiers & Qt.ControlModifier) && page.adding === "") {
            CalendarAccounts.requestSync();
            event.accepted = true;
        }
    }

    SetColumn {
        SetSection { text: "accounts" }

        Repeater {
            model: CalendarAccounts.accounts
            SetRichRow {
                id: accountRow
                required property var modelData
                readonly property var line: page.statusLine(modelData)
                richName: accountRow.ink(page.kindInk(modelData.kind), modelData.kind) + " " + accountRow.escapeText(modelData.kind === "ics" ? modelData.name : CalendarAccounts.masked(modelData.user))
                description: page.description(modelData)

                function escapeText(t) {
                    return String(t).replace(/&/g, "&amp;").replace(/</g, "&lt;");
                }

                extra: [
                    H.HText {
                        text: accountRow.line.text
                        role: "caption"
                        ink: accountRow.line.ink
                    },
                    H.InkButton {
                        visible: accountRow.modelData.kind !== "ics"
                        text: "edit"
                        flat: true
                        onClicked: page.startAdding(accountRow.modelData.kind, accountRow.modelData.id)
                    },
                    H.InkButton {
                        text: "remove"
                        flat: true
                        onClicked: CalendarAccounts.remove(accountRow.modelData.id)
                    }
                ]
            }
        }
        SetNote {
            visible: CalendarAccounts.accounts.length === 0
            narrator: true
            text: "No calendars yet. The month on the dashboard is blank, waiting."
        }

        Row {
            Layout.topMargin: 10
            Layout.bottomMargin: 10
            spacing: 8
            visible: page.adding === ""
            H.InkButton { text: "add icloud"; ink: "c3"; onClicked: page.startAdding("icloud") }
            H.InkButton { text: "add caldav"; ink: "c1"; onClicked: page.startAdding("caldav") }
            H.InkButton { text: "add .ics url"; ink: "c1"; onClicked: page.startAdding("ics") }
        }

        // the sign-in form
        ColumnLayout {
            Layout.fillWidth: true
            Layout.topMargin: 10
            visible: page.adding !== ""
            spacing: 0

            SetSection {
                text: page.editingId !== "" ? "sign in again" : page.adding === "icloud" ? "add icloud" : page.adding === "caldav" ? "add caldav" : "add .ics url"
            }
            SetNote {
                visible: page.adding === "icloud"
                text: "Make an app-specific password at account.apple.com › Sign-In and Security › App-Specific Passwords. Your Apple ID password will not work here, and the app-specific one goes to the keyring."
            }
            SetText {
                id: fieldName
                searchable: false
                visible: page.adding === "ics"
                name: "name"
                placeholder: "public holidays"
                value: ""
            }
            SetText {
                id: fieldUrl
                searchable: false
                visible: page.adding === "caldav" || page.adding === "ics"
                name: page.adding === "ics" ? "url" : "server"
                placeholder: page.adding === "ics" ? "https://… or webcal://…" : "https://dav.example.com/"
                columns: 32
                value: ""
                onAccepted: if (page.adding === "ics") page.submit()
            }
            SetText {
                id: fieldUser
                searchable: false
                visible: page.adding === "icloud" || page.adding === "caldav"
                name: page.adding === "icloud" ? "apple id" : "user"
                placeholder: page.adding === "icloud" ? "you@icloud.com" : ""
                columns: 28
                value: ""
            }
            SetText {
                id: fieldPassword
                searchable: false
                visible: page.adding === "icloud" || page.adding === "caldav"
                name: page.adding === "icloud" ? "app-specific password" : "password"
                placeholder: page.adding === "icloud" ? "xxxx-xxxx-xxxx-xxxx" : ""
                secret: true
                columns: 22
                value: ""
                onAccepted: page.submit()
            }
            SetNote {
                visible: page.error !== "" || page.busy
                text: page.busy ? "Asking the server…" : page.error
                color: page.busy ? H.HTokens.muted : H.HTokens.love
            }
            Row {
                Layout.topMargin: 10
                Layout.bottomMargin: 10
                spacing: 8
                H.InkButton {
                    text: page.adding === "ics" ? "subscribe" : "sign in"
                    ink: "c3"
                    enabled: !page.busy && (page.adding === "icloud" || fieldUrl.text !== "")
                    onClicked: page.submit()
                }
                H.InkButton { text: "cancel"; ink: "c1"; onClicked: page.cancel() }
            }
        }

        SetSection { text: "sync" }
        SetNumber {
            name: "every"
            key: "calendars.syncInterval"
            from: 5
            to: 240
            stepSize: 5
            suffix: "m"
        }
        SetRow {
            name: "new events go to"
            description: "Events you add from the dashboard are written back to that calendar."
            H.Segmented {
                visible: CalendarAccounts.newEventTargets.length <= 3
                model: CalendarAccounts.newEventTargets
                currentIndex: Math.max(0, CalendarAccounts.newEventTargets.indexOf(Config.options.calendars.newEventsTo))
                onActivated: i => Config.options.calendars.newEventsTo = CalendarAccounts.newEventTargets[i]
            }
            H.Spin {
                visible: CalendarAccounts.newEventTargets.length > 3
                value: Math.max(0, CalendarAccounts.newEventTargets.indexOf(Config.options.calendars.newEventsTo))
                from: 0
                to: CalendarAccounts.newEventTargets.length - 1
                text: CalendarAccounts.newEventTargets[value] ?? "local"
                onMoved: v => Config.options.calendars.newEventsTo = CalendarAccounts.newEventTargets[v]
            }
        }
    }

    SetColumn {
        Repeater {
            model: CalendarAccounts.accounts.filter(a => a.kind !== "ics")

            ColumnLayout {
                id: accountCals
                required property var modelData
                Layout.fillWidth: true
                spacing: 0

                SetSection { text: `${accountCals.modelData.name} calendars` }

                Repeater {
                    model: accountCals.modelData.calendars ?? []
                    SetRichRow {
                        id: calRow
                        required property var modelData
                        readonly property bool reminders: !(modelData.components ?? ["VEVENT"]).includes("VEVENT")
                        richName: calRow.ink(page.calendarInk(modelData.color), "■") + " " + String(modelData.name).replace(/&/g, "&amp;").replace(/</g, "&lt;")
                        description: calRow.reminders ? (accountCals.modelData.kind === "icloud"
                                ? "A reminders list. The current iCloud Reminders are not reachable over CalDAV, so it may stay empty; it could feed the todo widget instead."
                                : "A tasks list. It could feed the todo widget instead.")
                            : !modelData.writable ? "Read only: shared with you, or a subscription." : ""
                        extra: [
                            H.Chip {
                                interactive: true
                                on: calRow.modelData.shown
                                text: calRow.modelData.shown ? "shown" : "hidden"
                                onClicked: CalendarAccounts.setShown(accountCals.modelData.id, calRow.modelData.href, !calRow.modelData.shown)
                            }
                        ]
                    }
                }
            }
        }

        SetSection { text: "where events show" }
        SetNote {
            text: "dashboard calendar, quick panel calendar, countdown widget, the desktop clock's narration line, and a toast 10 minutes before."
        }
    }
}
