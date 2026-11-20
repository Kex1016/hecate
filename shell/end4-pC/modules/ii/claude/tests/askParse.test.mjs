// Unit check for the ask panel's answer parsing and what it sends.
//   node --test shell/end4-pC/modules/ii/claude/tests/askParse.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const src = readFileSync(new URL("../AskParse.js", import.meta.url), "utf8").replace(/^\.pragma library\s*$/m, "");
const A = {};
vm.runInNewContext(src + "\nthis.A = { blocks, script, styled, summary, prompt, textBlock, event, explain, pathFromTitle, lineCount };", A);
const { blocks, script, styled, summary, prompt, textBlock, event, explain, pathFromTitle, lineCount } = A.A;
const plain = v => JSON.parse(JSON.stringify(v));

const answer = [
    "The build imports `gen/cakeos.nix`, and `gen/` is gitignored.",
    "",
    "Copy a preset in as a stand-in, then build again:",
    "",
    "```sh",
    "mkdir -p gen",
    "cp gen-presets/hecate.nix gen/cakeos.nix",
    "git add -N gen/cakeos.nix",
    "```",
    "",
    "```nix",
    "imports = [ ./gen/cakeos.nix ];",
    "```",
].join("\n");

test("text and code blocks", () => {
    const b = plain(blocks(answer));
    assert.equal(b.length, 4);
    assert.equal(b[0].kind, "text");
    assert.equal(b[1].text, "Copy a preset in as a stand-in, then build again:");
    assert.deepEqual(b[2], { kind: "code", lang: "sh", lines: ["mkdir -p gen", "cp gen-presets/hecate.nix gen/cakeos.nix", "git add -N gen/cakeos.nix"], runnable: true, open: false, command: 0 });
    assert.equal(b[3].runnable, false);
    assert.equal(b[3].command, -1);
    assert.equal(script(b[2]), "mkdir -p gen\ncp gen-presets/hecate.nix gen/cakeos.nix\ngit add -N gen/cakeos.nix");
});

test("a fence still being written is open and does not run", () => {
    const b = plain(blocks("Try:\n\n```bash\nnix flake check"));
    assert.equal(b[1].open, true);
    assert.deepEqual(b[1].lines, ["nix flake check"]);
});

test("console transcripts keep only the commands", () => {
    const b = plain(blocks("```console\n$ ls -la\ntotal 0\n$ pwd\n/home\n```"));
    assert.deepEqual(b[0].lines, ["ls -la", "pwd"]);
});

test("styled text", () => {
    assert.equal(styled("use `a<b` and **this**", "#c4a7e7"), "use <font color=\"#c4a7e7\">a&lt;b</font> and <b>this</b>");
    assert.equal(styled("an `open tick", "#fff"), "an `open tick");
});

test("summary says what goes", () => {
    assert.equal(summary([{ id: "window" }, { id: "selection", lines: 6 }]), "window title + 6 lines");
    assert.equal(summary([]), "question only");
    assert.equal(summary([{ id: "region", width: 520, height: 300 }, { id: "screenshot" }]), "520×300 region + screenshot");
    assert.equal(summary([{ id: "clipboard", lines: 1 }]), "clipboard");
    assert.equal(summary([{ id: "clipboard", image: true }]), "clipboard image");
});

test("the prompt holds only what was let through", () => {
    assert.equal(prompt("  why  ", {}), "why");
    const p = prompt("why does this fail", { window: { appId: "kitty", title: "~/src/cakeos" }, selection: "error: x" });
    assert.match(p, /^Focused window: kitty - ~\/src\/cakeos\n\nSelected text in that window:\n<selection>\nerror: x\n<\/selection>\n\nQuestion: why does this fail$/);
    assert.doesNotMatch(p, /clipboard|screenshot/i);
    assert.deepEqual(JSON.parse(textBlock("a \"b\"\n")), { type: "text", text: "a \"b\"\n" });
});

test("stream-json events", () => {
    assert.deepEqual(plain(event('{"type":"system","subtype":"init","session_id":"s1","model":"m"}')), { session: "s1", model: "m" });
    assert.deepEqual(plain(event('{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}},"session_id":"s1"}')), { session: "s1", delta: "Hi" });
    assert.deepEqual(plain(event('{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"}]},"session_id":"s1"}')), { session: "s1", full: "Hello" });
    assert.deepEqual(plain(event('{"type":"result","subtype":"success","is_error":false,"result":"Hello","session_id":"s1"}')), { session: "s1", done: true, result: "Hello" });
    const err = plain(event('{"type":"result","subtype":"success","is_error":true,"result":"Not logged in · Please run /login"}'));
    assert.equal(err.error, "Not logged in · Please run /login");
    assert.match(explain(err.error), /not logged in/);
    assert.equal(event("not json"), null);
});

test("window titles", () => {
    assert.equal(pathFromTitle("~/src/cakeos"), "~/src/cakeos");
    assert.equal(pathFromTitle("cakes@universe: ~/src"), "~/src");
    assert.equal(pathFromTitle("nvim /etc/nixos/configuration.nix"), "/etc/nixos/configuration.nix");
    assert.equal(pathFromTitle("Firefox"), "");
    assert.equal(lineCount("a\nb\n"), 2);
    assert.equal(lineCount(""), 0);
});
