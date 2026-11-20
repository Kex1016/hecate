pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs.services

// Every setting on the dashboard's pages, read from the page files
// themselves: each SetToggle, SetChoice, SetNumber, SetText, SetAction or
// SetInfo with a literal `name`, under the SetSection before it. The
// dashboard's grep and the launcher search it; SettingsQuickControls turns
// an entry back into a control the launcher can show inline.
Singleton {
    id: root

    // { <page id>: [{ section, label, description, type, key, hecateKey, values, labels,
    //                 from, to, stepSize, divisor, suffix, inverted }] }
    property var entriesByPage: ({})

    readonly property var rowTypes: ["SetToggle", "SetChoice", "SetNumber", "SetText", "SetAction", "SetInfo", "SetRow", "SetRichRow"]
    readonly property var props: ["name", "description", "key", "hecateKey", "values", "labels", "from", "to", "stepSize", "divisor", "suffix", "inverted", "searchable"]

    // A QML literal: "string", number, true/false or a JSON-style array. Anything
    // else (a binding) reads as undefined.
    function literal(text) {
        const t = text.trim().replace(/;$/, "").trim();
        if (!/^(".*"|-?\d+(\.\d+)?|true|false|\[.*\])$/.test(t)) return undefined;
        try {
            return JSON.parse(t);
        } catch (e) {
            return undefined;
        }
    }

    function parsePage(source) {
        const entries = [];
        let section = "";
        let depth = 0;
        let cur = null;
        let curDepth = -1;
        for (const line of source.split("\n")) {
            const sec = line.match(/\bSetSection\s*\{\s*text:\s*("(?:[^"\\]|\\.)*")/);
            if (sec) section = root.literal(sec[1]) ?? section;

            const open = line.match(/^\s*(Set[A-Za-z]+)\s*\{/);
            if (open && root.rowTypes.includes(open[1]) && cur === null) {
                cur = { type: open[1], section };
                curDepth = depth;
            }
            if (cur !== null) {
                // `name: "x"` on its own line, or `{ name: "x"; text: ... }` on one line
                const body = line.replace(/^\s*Set[A-Za-z]+\s*\{/, "").replace(/\}\s*$/, "");
                for (const part of body.split(/;(?=\s*\w+\s*:)/)) {
                    const m = part.match(/^\s*(\w+)\s*:\s*(.+)$/);
                    if (m && root.props.includes(m[1]) && cur[m[1]] === undefined) {
                        const v = root.literal(m[2]);
                        if (v !== undefined) cur[m[1]] = v;
                    }
                }
            }
            depth += (line.match(/\{/g) || []).length - (line.match(/\}/g) || []).length;
            if (cur !== null && depth <= curDepth) {
                if (typeof cur.name === "string" && cur.name !== "" && cur.searchable !== false)
                    entries.push(Object.assign({ label: cur.name }, cur));
                cur = null;
                curDepth = -1;
            }
        }
        return entries;
    }

    function indexPage(pageId, source) {
        const next = Object.assign({}, root.entriesByPage);
        next[pageId] = root.parsePage(source);
        root.entriesByPage = next;
    }

    function count(pageId) {
        return (root.entriesByPage[pageId] ?? []).filter(e => e.type !== "SetInfo").length;
    }

    function entry(pageId, label) {
        return (root.entriesByPage[pageId] ?? []).find(e => e.label === label) ?? null;
    }

    // Entries whose words hold every word of the query, best first: the name
    // before its section, page or description.
    function search(query, limit) {
        const tokens = String(query).toLowerCase().trim().split(/\s+/).filter(t => t.length > 0);
        if (tokens.length === 0) return [];
        const results = [];
        for (const page of SettingsPages.pages) {
            const pageName = page.name;
            const pageWords = `${pageName} ${page.keywords ?? ""}`;
            const pageScore = tokens.every(t => pageWords.includes(t)) ? 3 : 0;
            if (pageScore > 0)
                results.push({ kind: "page", pageId: page.id, pageName, section: "", subsection: "", label: "", rawSection: "", rawSubsection: "", rawLabel: "", score: 50 });
            for (const e of root.entriesByPage[page.id] ?? []) {
                const label = e.label.toLowerCase();
                const hay = [label, (e.section ?? "").toLowerCase(), pageName, (e.description ?? "").toLowerCase()];
                if (!tokens.every(t => hay.some(h => h.includes(t)))) continue;
                let score = 0;
                for (const t of tokens) {
                    if (label === t) score += 40;
                    else if (label.startsWith(t)) score += 25;
                    else if (label.includes(t)) score += 15;
                    else if (hay[1].includes(t) || hay[2].includes(t)) score += 6;
                    else score += 2;
                }
                results.push({
                    kind: "option", pageId: page.id, pageName,
                    section: e.section ?? "", subsection: "", label: e.label, description: e.description ?? "",
                    rawSection: e.section ?? "", rawSubsection: "", rawLabel: e.label,
                    score
                });
            }
        }
        results.sort((a, b) => b.score - a.score);
        return limit ? results.slice(0, limit) : results;
    }

    Instantiator {
        model: SettingsPages.pages
        delegate: FileView {
            id: pageFile
            required property var modelData
            path: Quickshell.shellPath(modelData.path)
            printErrors: false
            onLoaded: root.indexPage(pageFile.modelData.id, pageFile.text())
        }
    }
}
