.pragma library

// Which MPRIS player the shell shows, as plain functions with no QML in them so
// they can be checked with node (see playerPriority.test.mjs).
//
// The rule: Spotify and Cider (cider.sh, an Apple Music frontend) come first,
// whichever of them played last. A browser, mpv or anything else is only
// picked when neither of the two is open. Open means the player is on the bus,
// playing or not.
//
// A player is a plain object:
//   identity, desktopEntry, dbusName   strings, any may be empty
//   isPlaying                          bool
//   stamp                              ms timestamp of its last start or stop, 0 if never
//   order                              its position on the bus, the last tie-break

var MUSIC_APPS = ["spotify", "cider"];

function haystack(p) {
    return ((p.identity || "") + " " + (p.desktopEntry || "") + " " + (p.dbusName || "")).toLowerCase();
}

// Spotify or Cider, whatever the packaging calls them (`Spotify`, `Cider`,
// `org.mpris.MediaPlayer2.spotify`, `cider.desktop`).
function isMusicApp(p) {
    var h = haystack(p);
    for (var i = 0; i < MUSIC_APPS.length; i++) {
        if (h.indexOf(MUSIC_APPS[i]) !== -1) return true;
    }
    return false;
}

// Every player, best first: Spotify and Cider before the rest; inside each
// group the one playing now, then the one that played most recently, then bus order.
function rank(players) {
    return players.slice().sort(function (a, b) {
        var am = isMusicApp(a) ? 1 : 0, bm = isMusicApp(b) ? 1 : 0;
        if (am !== bm) return bm - am;
        var ap = a.isPlaying ? 1 : 0, bp = b.isPlaying ? 1 : 0;
        if (ap !== bp) return bp - ap;
        var as = a.stamp || 0, bs = b.stamp || 0;
        if (as !== bs) return bs - as;
        return (a.order || 0) - (b.order || 0);
    });
}

// The player to show with no manual choice, or null with nothing open.
function pick(players) {
    var ranked = rank(players);
    return ranked.length > 0 ? ranked[0] : null;
}

// The short lowercase name on a player chip: `spotify`, `cider`, `zen`, `mpv`.
function shortName(p) {
    var name = (p.identity || "").toLowerCase().trim();
    if (name.length === 0) {
        var bus = (p.dbusName || "").toLowerCase().replace(/^org\.mpris\.mediaplayer2\./, "");
        name = bus.replace(/\.instance\d+$/, "");
    }
    name = name.replace(/^mozilla\s+/, "").replace(/\s+(web\s+)?browser$/, "").replace(/\s+media\s+player$/, "");
    var first = name.split(/\s+/)[0];
    return first.length > 0 ? first : "player";
}
