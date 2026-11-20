pragma Singleton

import QtQuick
import Quickshell
import qs.modules.common
import qs.services

// Inline controls for the launcher's settings results, made from the search
// index's entries: a SetToggle becomes a switch, a SetChoice a select, a
// SetNumber a spin. Only settings kept in the shell's config get one; the
// compositor's (hecate pages) open their page instead.
//   { type: "switch", get, set } | { type: "spin", get, set, from, to, stepSize }
//   | { type: "select", get, set, options: [{ displayName, value }] }
Singleton {
    id: root

    function read(path) {
        return path.split(".").reduce((o, k) => o?.[k], Config.options);
    }

    function make(e) {
        if (!e || !e.key) return null;
        const key = e.key;
        if (e.type === "SetToggle") {
            const inverted = e.inverted === true;
            return { type: "switch", get: () => (root.read(key) === true) !== inverted, set: v => Config.setNestedValue(key, v !== inverted) };
        }
        if (e.type === "SetChoice" && Array.isArray(e.values)) {
            const labels = Array.isArray(e.labels) ? e.labels : e.values.map(String);
            return {
                type: "select",
                get: () => root.read(key),
                set: v => Config.setNestedValue(key, v),
                options: e.values.map((v, i) => ({ displayName: labels[i] ?? String(v), value: v }))
            };
        }
        if (e.type === "SetNumber") {
            const scale = e.divisor ?? 1;
            return {
                type: "spin",
                get: () => Math.round((Number(root.read(key)) || 0) / scale * 100) / 100,
                set: v => Config.setNestedValue(key, v * scale),
                from: e.from ?? 0, to: e.to ?? 100, stepSize: e.stepSize ?? 1
            };
        }
        return null;
    }

    function find(pageId, rawSection, rawSubsection, rawLabel) {
        return root.make(SettingsSearchIndex.entry(pageId, rawLabel));
    }
}
