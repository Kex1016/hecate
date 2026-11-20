// Unit check for the launcher's matching and ranking.
//   node --test shell/end4-pC/modules/ii/overview/tests/launcherMatch.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const src = readFileSync(new URL("../LauncherMatch.js", import.meta.url), "utf8").replace(/^\.pragma library\s*$/m, "");
const M = {};
vm.runInNewContext(src + "\nthis.M = { parse, prefixFor, score, normalizeAliases, rank, commandAliases, label, age, looksLikeMath };", M);
const { parse, prefixFor, normalizeAliases, rank, commandAliases, label, age, looksLikeMath } = M.M;

const apps = [
    { id: "firefox.desktop", name: "Firefox", genericName: "Web Browser", keywords: ["internet", "www"] },
    { id: "firewall-config.desktop", name: "Firewall", genericName: "Firewall configuration", keywords: [] },
    { id: "soundfire.desktop", name: "Soundfire", genericName: "Audio", keywords: [] },
    { id: "yazi.desktop", name: "yazi", genericName: "File Manager", keywords: ["files", "fm"] },
    { id: "org.kde.dolphin.desktop", name: "Dolphin", genericName: "File Manager", keywords: ["files", "explorer"] },
    { id: "zathura.desktop", name: "Zathura", genericName: "Document Viewer", keywords: ["pdf"] },
    { id: "files.desktop", name: "Files Backup", genericName: "Backup", keywords: [] },
];

test("prefixes", () => {
    assert.deepEqual({ ...parse("v foo") }, { mode: "clipboard", prefix: "v ", text: "foo" });
    assert.equal(parse("vlc").mode, "search");
    assert.equal(parse("tr hello").mode, "translate");
    assert.equal(parse("transmission").mode, "search");
    assert.equal(parse(">ls -la").text, "ls -la");
    assert.equal(parse("=2^10").mode, "calc");
    assert.equal(parse(":star").mode, "emoji");
    assert.equal(parse("/notes").mode, "files");
    assert.equal(parse("? what").mode, "claude");
    assert.equal(parse("").mode, "search");
    assert.equal(prefixFor("translate"), "tr ");
});

test("an exact user alias ranks first and says which alias matched", () => {
    const aliases = normalizeAliases({ files: "org.kde.dolphin" });
    const r = rank(apps, "files", aliases);
    assert.equal(r[0].app.name, "Dolphin");
    assert.equal(r[0].alias, "files");
    assert.equal(r[0].field, "alias");
    assert.equal(label(r[0].app.name, r[0].alias), "Dolphin · files");
    // the exact keyword match comes next, ahead of the app whose name starts with it
    assert.equal(r[1].app.name, "yazi");
    assert.equal(label(r[1].app.name, r[1].alias), "yazi · files");
    assert.equal(r[2].app.name, "Files Backup");
    assert.equal(r[2].alias, "");
});

test("an exact alias beats an exact name", () => {
    const aliases = normalizeAliases({ firefox: "zathura" });
    const r = rank(apps, "firefox", aliases);
    assert.equal(r[0].app.name, "Zathura");
    assert.equal(r[1].app.name, "Firefox");
});

test("name prefix beats substring, scattered letters come last", () => {
    const r = rank(apps, "fir", {});
    assert.deepEqual([...r.map(x => x.app.name)], ["Firefox", "Firewall", "Soundfire"]);
    const s = rank(apps, "ffx", {});
    assert.equal(s[0].app.name, "Firefox");
});

test("desktop-entry keywords and generic names match", () => {
    assert.equal(rank(apps, "pdf", {})[0].app.name, "Zathura");
    assert.equal(rank(apps, "pdf", {})[0].alias, "pdf");
    const browser = rank(apps, "web browser", {});
    assert.equal(browser[0].app.name, "Firefox");
    assert.equal(browser[0].alias, "");
});

test("alias config forms and targets", () => {
    const a = normalizeAliases([{ alias: "Music", app: "spotify" }, { alias: "music", target: "cider.desktop" }]);
    assert.deepEqual([...a.music], ["spotify", "cider.desktop"]);
    const b = normalizeAliases({ dl: ["org.qbittorrent.qBittorrent"] });
    const qb = [{ id: "org.qbittorrent.qBittorrent.desktop", name: "qBittorrent", keywords: [] }];
    assert.equal(rank(qb, "dl", b)[0].alias, "dl");
    // by the last reverse-DNS part too
    assert.equal(rank(qb, "dl", normalizeAliases({ dl: "qbittorrent" }))[0].app.name, "qBittorrent");
    assert.deepEqual({ ...normalizeAliases(null) }, {});
});

test("aliases that are not apps run as commands", () => {
    const aliases = normalizeAliases({ top: "kitty -e btop", files: "yazi" });
    assert.deepEqual([...commandAliases(apps, "top", aliases).map(x => x.command)], ["kitty -e btop"]);
    assert.equal(commandAliases(apps, "files", aliases).length, 0);
});

test("no query, no results", () => {
    assert.equal(rank(apps, "  ", {}).length, 0);
    assert.equal(rank(apps, "qqqq", {}).length, 0);
});

test("ages and math detection", () => {
    assert.equal(age(5), "now");
    assert.equal(age(240), "4m");
    assert.equal(age(2 * 86400), "2 days ago");
    assert.ok(looksLikeMath("2+2"));
    assert.ok(looksLikeMath("10 kg to lb"));
    assert.ok(!looksLikeMath("2048 game"));
    assert.ok(!looksLikeMath("firefox"));
});
