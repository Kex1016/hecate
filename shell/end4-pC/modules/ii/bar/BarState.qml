pragma Singleton
import QtQuick
import Quickshell

// What the bar's segments share: which segment has its window open (one at
// a time, over every screen), and the short app names the compositor's
// window frames use, so the bar and the frames say the same thing.
Singleton {
    id: root

    // the segment whose hover window is open, or null
    property Item openSegment: null

    // The last part of a reverse-DNS app id, in lower case
    // (org.gnome.Nautilus is nautilus), as hecate's render/label.rs does it.
    function appName(appId) {
        const id = String(appId ?? "").replace(/\.desktop$/, "");
        const parts = id.split(".");
        const last = parts[parts.length - 1];
        // a letter in any script has an upper and a lower case
        const letter = last.length > 0 && last[0].toLowerCase() !== last[0].toUpperCase();
        const name = parts.length >= 3 && letter ? last : id;
        return root.oneLine(name).toLowerCase();
    }

    // control characters and runs of white space become single spaces
    function oneLine(s) {
        return String(s ?? "").split(/[\s\x00-\x1f\x7f-\x9f]+/).filter(w => w.length > 0).join(" ");
    }

    // A window as its frame labels it: "app: title", or whichever is there.
    function windowLabel(appId, title) {
        const app = root.appName(appId);
        const t = root.oneLine(title);
        if (app.length > 0 && t.length > 0 && t.toLowerCase() !== app)
            return app + ": " + t;
        return app.length > 0 ? app : t;
    }

    // 5h 40m, 12m
    function duration(seconds) {
        const s = Math.max(0, Math.round(seconds));
        const h = Math.floor(s / 3600);
        const m = Math.floor((s % 3600) / 60);
        return h > 0 ? h + "h " + m + "m" : m + "m";
    }

    // 1:23
    function clock(seconds) {
        const s = Math.max(0, Math.floor(seconds));
        return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0");
    }
}
