// Unit check for the player priority: node playerPriority.test.mjs
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";

const src = readFileSync(fileURLToPath(new URL("./playerPriority.js", import.meta.url)), "utf8").replace(/^\.pragma library\s*$/m, "");
const { rank, pick, shortName, isMusicApp } = new Function(`${src}; return { rank, pick, shortName, isMusicApp };`)();

let order = 0;
const p = (identity, o = {}) => ({ identity, desktopEntry: "", dbusName: `org.mpris.MediaPlayer2.${identity.toLowerCase()}`, isPlaying: false, stamp: 0, order: order++, ...o });
const names = (list) => list.map((x) => x.identity);
const reset = () => { order = 0; };
let n = 0;
const check = (title, fn) => { reset(); fn(); n++; console.log(`ok ${n} ${title}`); };

check("nothing open picks nothing", () => assert.equal(pick([]), null));

check("a paused Spotify beats a playing browser", () => {
    const list = [p("Firefox", { isPlaying: true, stamp: 900 }), p("Spotify", { stamp: 100 })];
    assert.equal(pick(list).identity, "Spotify");
});

check("a paused Cider beats a playing mpv", () => {
    const list = [p("mpv", { isPlaying: true, stamp: 900 }), p("Cider", { stamp: 100 })];
    assert.equal(pick(list).identity, "Cider");
});

check("Spotify and Cider both paused: the one that played last", () => {
    assert.equal(pick([p("Spotify", { stamp: 100 }), p("Cider", { stamp: 200 })]).identity, "Cider");
    reset();
    assert.equal(pick([p("Cider", { stamp: 200 }), p("Spotify", { stamp: 300 })]).identity, "Spotify");
});

check("the one playing now beats one that played last", () => {
    const list = [p("Cider", { stamp: 500 }), p("Spotify", { isPlaying: true, stamp: 400 })];
    assert.equal(pick(list).identity, "Spotify");
});

check("both playing: the one that started last", () => {
    const list = [p("Spotify", { isPlaying: true, stamp: 100 }), p("Cider", { isPlaying: true, stamp: 200 })];
    assert.equal(pick(list).identity, "Cider");
});

check("neither open: a playing browser, then the most recent, then bus order", () => {
    assert.equal(pick([p("mpv", { stamp: 300 }), p("Zen Browser", { isPlaying: true, stamp: 100 })]).identity, "Zen Browser");
    reset();
    assert.equal(pick([p("mpv", { stamp: 100 }), p("Zen Browser", { stamp: 300 })]).identity, "Zen Browser");
    reset();
    assert.equal(pick([p("mpv"), p("Zen Browser")]).identity, "mpv");
});

check("the whole list puts Spotify and Cider before browsers and mpv", () => {
    const list = [p("Zen Browser", { isPlaying: true, stamp: 900 }), p("mpv", { isPlaying: true, stamp: 800 }), p("Cider", { stamp: 50 }), p("Spotify", { stamp: 60 })];
    assert.deepEqual(names(rank(list)), ["Spotify", "Cider", "Zen Browser", "mpv"]);
});

check("rank does not change its input", () => {
    const list = [p("mpv"), p("Spotify")];
    rank(list);
    assert.deepEqual(names(list), ["mpv", "Spotify"]);
});

check("an app is found by identity, desktop entry or bus name", () => {
    assert.ok(isMusicApp({ identity: "Spotify" }));
    assert.ok(isMusicApp({ identity: "Cider" }));
    assert.ok(isMusicApp({ desktopEntry: "cider" }));
    assert.ok(isMusicApp({ dbusName: "org.mpris.MediaPlayer2.spotify" }));
    assert.ok(!isMusicApp({ identity: "Mozilla Firefox", dbusName: "org.mpris.MediaPlayer2.firefox.instance1234" }));
    assert.ok(!isMusicApp({ identity: "mpv" }));
});

check("chip names", () => {
    assert.equal(shortName({ identity: "Spotify" }), "spotify");
    assert.equal(shortName({ identity: "Cider" }), "cider");
    assert.equal(shortName({ identity: "Zen Browser" }), "zen");
    assert.equal(shortName({ identity: "Mozilla Firefox" }), "firefox");
    assert.equal(shortName({ identity: "mpv" }), "mpv");
    assert.equal(shortName({ identity: "", dbusName: "org.mpris.MediaPlayer2.chromium.instance42" }), "chromium");
    assert.equal(shortName({}), "player");
});

console.log(`${n} checks passed`);
