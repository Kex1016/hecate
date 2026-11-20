pragma Singleton

import QtQuick
import Quickshell
import qs.services

// The dashboard's settings pages, in rail order. `group` splits the rail:
// the shell's own pages, then hecate's (compositor settings, only under
// hecate), then about. Each page is a file in modules/ii/settings/pages/.
Singleton {
    id: root

    readonly property string pagesDir: "modules/ii/settings/pages/"
    readonly property bool hecate: WM.compositor === "hecate"

    readonly property var pages: {
        const list = [
            { id: "general",      name: "general",      group: "shell",  keywords: "name night light battery volume", file: "GeneralPage.qml" },
            { id: "bar",          name: "bar",          group: "shell",  file: "BarPage.qml" },
            { id: "desktop",      name: "desktop",      group: "shell",  keywords: "wallpaper background blobs clock lock notifications time", file: "DesktopPage.qml" },
            { id: "panels",       name: "panels",       group: "shell",  keywords: "launcher alias search translate osd screenshot recording", file: "PanelsPage.qml" },
            { id: "calendars",    name: "calendars",    group: "shell",  keywords: "icloud caldav ics webcal reminders events", file: "CalendarsPage.qml" },
            { id: "services",     name: "services",     group: "shell",  keywords: "claude weather updates music", file: "ServicesPage.qml" },
        ];
        if (root.hecate) list.push(
            { id: "windows",      name: "windows",      group: "hecate", keywords: "gaps placement decorations snapping", file: "WindowsPage.qml" },
            { id: "focus",        name: "focus",        group: "hecate", file: "FocusPage.qml" },
            { id: "effects",      name: "effects",      group: "hecate", keywords: "motion animations tearing scanout", file: "EffectsPage.qml" },
            { id: "displays",     name: "displays",     group: "hecate", keywords: "monitor screen resolution refresh scale rotation vrr", file: "DisplaysPage.qml" },
            { id: "mouse",        name: "mouse",        group: "hecate", keywords: "pointer touchpad acceleration scroll", file: "MousePage.qml" },
            { id: "idle",         name: "idle",         group: "hecate", keywords: "lock screen off suspend", file: "IdlePage.qml" },
            { id: "login screen", name: "login screen", group: "hecate", keywords: "greeter greetd", file: "LoginScreenPage.qml" },
        );
        list.push({ id: "about", name: "about", group: "end", keywords: "system version cakeos updates", file: "AboutPage.qml" });
        return list.map(page => Object.assign({}, page, {
            component: Qt.resolvedUrl("../" + root.pagesDir + page.file),
            path: root.pagesDir + page.file
        }));
    }

    function byId(id) {
        return root.pages.find(page => page.id === id) ?? null;
    }
}
