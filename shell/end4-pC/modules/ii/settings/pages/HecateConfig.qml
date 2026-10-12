import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.modules.common.functions
import qs.modules.common.widgets
import qs.services

/**
 * hecate settings. Only what you change here is saved, to ~/.config/hecate/shell.toml
 * (with the edits kept in shell.json next to it). The Home Manager module includes that file,
 * so these win over the Nix config; Reset removes them.
 */
ContentPage {
    id: page
    forceWidth: true

    readonly property string dir: `${Directories.config}/hecate`
    // Effective config from the compositor, and the values edited here ("border.width" -> 2).
    property var effective: ({})
    property var edits: ({})
    property bool loaded: false
    // Outputs as `hecate msg -j outputs` reports them, by name.
    property var outputs: []

    function value(path, fallback) {
        if (page.edits[path] !== undefined) return page.edits[path];
        const v = path.split(".").reduce((o, k) => o?.[k], page.effective);
        return v ?? fallback;
    }

    function set(path, v) {
        if (!page.loaded || page.value(path, undefined) === v) return;
        const e = Object.assign({}, page.edits);
        e[path] = v;
        page.edits = e;
        page.save();
    }

    // Drop an edit, so the Nix config (or hecate's default) applies again.
    function unset(path) {
        if (!page.loaded || page.edits[path] === undefined) return;
        const e = Object.assign({}, page.edits);
        delete e[path];
        page.edits = e;
        page.save();
    }

    // Bare TOML key where allowed, quoted otherwise ("type:pointer", "*").
    function tomlKey(k) {
        return /^[A-Za-z0-9_-]+$/.test(k) ? k : JSON.stringify(k);
    }

    function toToml(edits) {
        const sections = {};
        for (const [path, v] of Object.entries(edits)) {
            const parts = path.split(".").map(page.tomlKey);
            const key = parts.pop();
            const section = parts.join(".");
            (sections[section] = sections[section] ?? []).push(`${key} = ${JSON.stringify(v)}`);
        }
        let out = "# Written by the shell's hecate settings page. Edit there, or delete this file.\n";
        for (const [section, lines] of Object.entries(sections))
            out += `\n[${section}]\n` + lines.join("\n") + "\n";
        return out;
    }

    function save() {
        jsonFile.setText(JSON.stringify(page.edits, null, 2));
        tomlFile.setText(page.toToml(page.edits));
        reloadTimer.restart();
    }

    // The exact mode string hecate writes for a mode, so it picks that mode again.
    function modeValue(m) {
        return `${m.width}x${m.height}@${(m.refresh_rate / 1000).toFixed(3)}`;
    }

    // Every mode of an output, largest and fastest first, behind a "default" entry.
    function modeOptions(o) {
        const seen = {};
        const modes = (o.modes ?? []).filter(m => {
            const v = page.modeValue(m);
            if (seen[v]) return false;
            seen[v] = true;
            return true;
        }).sort((a, b) => (b.width * b.height - a.width * a.height) || (b.refresh_rate - a.refresh_rate));
        return [{ displayName: Translation.tr("Default (Nix config, else the monitor's preferred)"), value: "" }]
            .concat(modes.map(m => ({
                displayName: `${m.width}×${m.height} @ ${(m.refresh_rate / 1000).toFixed(2)} Hz` + (m.is_preferred ? Translation.tr(" (preferred)") : ""),
                value: page.modeValue(m),
            })));
    }

    function currentMode(o) {
        const edited = page.edits[`outputs.${o.name}.mode`];
        if (edited !== undefined) return edited;
        const m = o.current_mode !== null && o.current_mode !== undefined ? o.modes[o.current_mode] : null;
        return m ? page.modeValue(m) : "";
    }

    Component.onCompleted: {
        mkdirProc.running = true;
        configProc.running = true;
        outputsProc.running = true;
    }

    Process { id: mkdirProc; command: ["mkdir", "-p", page.dir] }
    Process {
        id: reloadProc
        command: ["hecate", "msg", "reload"]
        // A new mode changes what the outputs report.
        onExited: outputsTimer.restart()
    }
    Timer { id: outputsTimer; interval: 500; onTriggered: outputsProc.running = true }
    Process {
        id: outputsProc
        command: ["hecate", "msg", "-j", "outputs"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const map = JSON.parse(text);
                    page.outputs = Object.keys(map).sort().map(k => map[k]);
                } catch (e) { console.log("[HecateConfig] " + e); }
            }
        }
    }
    Timer { id: reloadTimer; interval: 300; onTriggered: reloadProc.running = true }

    Process {
        id: configProc
        command: ["hecate", "msg", "-j", "config"]
        stdout: StdioCollector {
            onStreamFinished: {
                try { page.effective = JSON.parse(text); } catch (e) { console.log("[HecateConfig] " + e); }
                jsonFile.reload();
            }
        }
    }

    FileView {
        id: jsonFile
        path: `${page.dir}/shell.json`
        onLoaded: {
            try { page.edits = JSON.parse(jsonFile.text()); } catch (e) { page.edits = {}; }
            page.loaded = true;
        }
        onLoadFailed: page.loaded = true
    }
    FileView { id: tomlFile; path: `${page.dir}/shell.toml` }

    Process {
        id: resetProc
        command: ["sh", "-c", `rm -f "${page.dir}/shell.toml" "${page.dir}/shell.json" && hecate msg reload`]
        onExited: { page.edits = {}; configProc.running = true; }
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 20

        NoticeBox {
            Layout.fillWidth: true
            text: Translation.tr("Changes here are saved to ~/.config/hecate/shell.toml and override your Nix config. Everything else stays in your Home Manager settings.")
        }

        ContentSection {
            icon: "crop_square"
            title: Translation.tr("Windows")

            ConfigSpinBox {
                icon: "border_style"
                text: Translation.tr("Border width")
                from: 0; to: 20; stepSize: 1
                value: page.value("border.width", 2)
                onValueChanged: page.set("border.width", value)
            }
            ConfigSpinBox {
                icon: "rounded_corner"
                text: Translation.tr("Corner radius")
                from: 0; to: 40; stepSize: 1
                // Per-corner radii from Nix show as their first corner.
                value: { const r = page.value("border.radius", 10); return typeof r === "number" ? r : (r?.top_left ?? 0); }
                onValueChanged: page.set("border.radius", value)
            }
            ConfigSwitch {
                buttonIcon: "content_cut"
                text: Translation.tr("Clip window contents to the rounded corners")
                checked: page.value("border.clip_content", true)
                onCheckedChanged: page.set("border.clip_content", checked)
            }
            ConfigSpinBox {
                icon: "space_bar"
                text: Translation.tr("Gaps when snapping and maximizing")
                from: 0; to: 64; stepSize: 1
                value: page.value("general.gaps", 0)
                onValueChanged: page.set("general.gaps", value)
            }
        }

        ContentSection {
            icon: "ads_click"
            title: Translation.tr("Focus")

            ConfigSwitch {
                buttonIcon: "mouse"
                text: Translation.tr("Focus follows the mouse")
                checked: page.value("focus.follow_mouse", false)
                onCheckedChanged: page.set("focus.follow_mouse", checked)
            }
            ConfigSwitch {
                buttonIcon: "flip_to_front"
                text: Translation.tr("Raise windows when focused")
                checked: page.value("focus.raise_on_focus", true)
                onCheckedChanged: page.set("focus.raise_on_focus", checked)
            }
        }

        ContentSection {
            icon: "animation"
            title: Translation.tr("Effects")

            ConfigSwitch {
                buttonIcon: "motion_photos_on"
                text: Translation.tr("Animations")
                checked: page.value("animations.enable", true)
                onCheckedChanged: page.set("animations.enable", checked)
            }

            ConfigSwitch {
                buttonIcon: "blur_on"
                text: Translation.tr("Blur behind panels and windows that ask for it")
                checked: page.value("blur.enable", true)
                onCheckedChanged: page.set("blur.enable", checked)
            }
        }

        ContentSection {
            icon: "monitor"
            title: Translation.tr("Displays")

            NoticeBox {
                Layout.fillWidth: true
                visible: page.outputs.length === 0
                text: Translation.tr("No displays reported by hecate.")
            }

            // Drag the monitors into place. Each one snaps to the nearest edge of another on
            // release; the result is saved as its position, like the spin boxes below.
            Rectangle {
                id: arrangement
                Layout.fillWidth: true
                Layout.preferredHeight: 240
                visible: page.outputs.length > 0
                radius: Appearance.rounding.small
                color: Appearance.colors.colLayer2

                readonly property real pad: 16
                readonly property var boxes: page.outputs.filter(o => o.logical).map(o => {
                    const p = page.value(`outputs.${o.name}.position`, [o.logical.x, o.logical.y]);
                    return { name: o.name, x: p[0], y: p[1], w: o.logical.width, h: o.logical.height,
                             label: `${o.name}\n${o.logical.width}×${o.logical.height}` };
                })
                // The view's origin and scale, frozen while dragging so nothing jumps.
                property bool dragging: false
                property real originX: 0
                property real originY: 0
                property real k: 0.05
                function refit() {
                    if (arrangement.dragging || arrangement.boxes.length === 0) return;
                    const minX = Math.min(...arrangement.boxes.map(b => b.x));
                    const minY = Math.min(...arrangement.boxes.map(b => b.y));
                    const spanW = Math.max(...arrangement.boxes.map(b => b.x + b.w)) - minX;
                    const spanH = Math.max(...arrangement.boxes.map(b => b.y + b.h)) - minY;
                    // Leave room around the layout to drag a monitor past the others.
                    arrangement.k = Math.min((arrangement.width - 2 * arrangement.pad) / (spanW * 1.5),
                                             (arrangement.height - 2 * arrangement.pad) / (spanH * 1.5));
                    arrangement.originX = minX - (arrangement.width / arrangement.k - spanW) / 2;
                    arrangement.originY = minY - (arrangement.height / arrangement.k - spanH) / 2;
                }
                onBoxesChanged: refit()
                onWidthChanged: refit()

                // Where `box` should land near logical (x, y): flush against another monitor's
                // edge when within 64 logical pixels, else where it was dropped.
                function snapped(box, x, y) {
                    const reach = 64;
                    let bestX = x, dx = reach, bestY = y, dy = reach;
                    for (const o of arrangement.boxes) {
                        if (o.name === box.name) continue;
                        for (const cx of [o.x + o.w, o.x - box.w, o.x, o.x + o.w - box.w])
                            if (Math.abs(cx - x) < dx) { dx = Math.abs(cx - x); bestX = cx; }
                        for (const cy of [o.y + o.h, o.y - box.h, o.y, o.y + o.h - box.h])
                            if (Math.abs(cy - y) < dy) { dy = Math.abs(cy - y); bestY = cy; }
                    }
                    return [Math.round(bestX), Math.round(bestY)];
                }

                Repeater {
                    model: arrangement.boxes

                    Rectangle {
                        id: monitorBox
                        required property var modelData
                        function placedX() { return (monitorBox.modelData.x - arrangement.originX) * arrangement.k; }
                        function placedY() { return (monitorBox.modelData.y - arrangement.originY) * arrangement.k; }
                        x: placedX()
                        y: placedY()
                        width: monitorBox.modelData.w * arrangement.k
                        height: monitorBox.modelData.h * arrangement.k
                        radius: Appearance.rounding.verysmall
                        color: dragArea.drag.active ? Appearance.colors.colPrimary : Appearance.colors.colPrimaryContainer
                        border.width: 1
                        border.color: Appearance.colors.colOutline

                        StyledText {
                            anchors.centerIn: parent
                            horizontalAlignment: Text.AlignHCenter
                            text: monitorBox.modelData.label
                            font.pixelSize: Appearance.font.pixelSize.smaller
                            color: dragArea.drag.active ? Appearance.colors.colOnPrimary : Appearance.colors.colOnPrimaryContainer
                        }

                        MouseArea {
                            id: dragArea
                            anchors.fill: parent
                            cursorShape: Qt.SizeAllCursor
                            drag.target: monitorBox
                            onPressed: arrangement.dragging = true
                            onReleased: {
                                const p = arrangement.snapped(monitorBox.modelData,
                                    monitorBox.x / arrangement.k + arrangement.originX,
                                    monitorBox.y / arrangement.k + arrangement.originY);
                                arrangement.dragging = false;
                                // Dragging replaced the bindings; put them back.
                                monitorBox.x = Qt.binding(() => monitorBox.placedX());
                                monitorBox.y = Qt.binding(() => monitorBox.placedY());
                                page.set(`outputs.${monitorBox.modelData.name}.position`, p);
                                arrangement.refit();
                            }
                        }
                    }
                }
            }

            Repeater {
                model: page.outputs

                ContentSubsection {
                    id: outputSection
                    required property var modelData
                    readonly property string name: modelData.name
                    readonly property string base: `outputs.${modelData.name}`
                    title: `${modelData.name}  ·  ${modelData.make} ${modelData.model}`.trim()

                    ConfigComboBox {
                        Layout.fillWidth: true
                        buttonIcon: "aspect_ratio"
                        text: Translation.tr("Resolution and refresh rate")
                        fieldWidth: 300
                        fixedWidth: true
                        model: page.modeOptions(outputSection.modelData)
                        currentValue: page.currentMode(outputSection.modelData)
                        onSelected: newValue => newValue === ""
                            ? page.unset(`${outputSection.base}.mode`)
                            : page.set(`${outputSection.base}.mode`, newValue)
                    }
                    ConfigComboBox {
                        Layout.fillWidth: true
                        buttonIcon: "zoom_in"
                        text: Translation.tr("Scale")
                        fieldWidth: 120
                        fixedWidth: true
                        model: [1, 1.25, 1.5, 1.75, 2, 2.5, 3].map(v => ({ displayName: `${v * 100}%`, value: v }))
                        currentValue: page.value(`${outputSection.base}.scale`, outputSection.modelData.logical?.scale ?? 1)
                        onSelected: newValue => page.set(`${outputSection.base}.scale`, newValue)
                    }
                    ConfigComboBox {
                        Layout.fillWidth: true
                        buttonIcon: "screen_rotation"
                        text: Translation.tr("Orientation")
                        fieldWidth: 220
                        fixedWidth: true
                        model: [
                            { displayName: Translation.tr("Landscape"), value: "normal" },
                            { displayName: Translation.tr("Portrait (90°)"), value: "90" },
                            { displayName: Translation.tr("Landscape, upside down"), value: "180" },
                            { displayName: Translation.tr("Portrait (270°)"), value: "270" },
                            { displayName: Translation.tr("Mirrored"), value: "flipped" },
                            { displayName: Translation.tr("Mirrored, 90°"), value: "flipped-90" },
                            { displayName: Translation.tr("Mirrored, 180°"), value: "flipped-180" },
                            { displayName: Translation.tr("Mirrored, 270°"), value: "flipped-270" },
                        ]
                        // hecate reports "flipped90" where the config spells "flipped-90".
                        currentValue: page.value(`${outputSection.base}.transform`,
                            (outputSection.modelData.logical?.transform ?? "normal").replace(/^flipped(\d+)$/, "flipped-$1"))
                        onSelected: newValue => page.set(`${outputSection.base}.transform`, newValue)
                    }
                    ConfigComboBox {
                        Layout.fillWidth: true
                        visible: outputSection.modelData.vrr_supported
                        buttonIcon: "speed"
                        text: Translation.tr("Variable refresh rate")
                        fieldWidth: 220
                        fixedWidth: true
                        model: [
                            { displayName: Translation.tr("Off"), value: "off" },
                            { displayName: Translation.tr("Fullscreen windows only"), value: "fullscreen" },
                            { displayName: Translation.tr("Always on"), value: "on" },
                        ]
                        currentValue: page.value(`${outputSection.base}.adaptive_sync`, "off")
                        onSelected: newValue => page.set(`${outputSection.base}.adaptive_sync`, newValue)
                    }
                    ConfigRow {
                        uniform: true
                        // Top-left corner in the global layout, in logical pixels.
                        ConfigSpinBox {
                            icon: "arrow_range"
                            text: Translation.tr("Position X")
                            from: -16384; to: 16384; stepSize: 10
                            value: page.value(`${outputSection.base}.position`, [outputSection.modelData.logical?.x ?? 0, 0])[0]
                            onValueChanged: {
                                const p = page.value(`${outputSection.base}.position`, [outputSection.modelData.logical?.x ?? 0, outputSection.modelData.logical?.y ?? 0]);
                                if (p[0] !== value) page.set(`${outputSection.base}.position`, [value, p[1]]);
                            }
                        }
                        ConfigSpinBox {
                            icon: "height"
                            text: Translation.tr("Position Y")
                            from: -16384; to: 16384; stepSize: 10
                            value: page.value(`${outputSection.base}.position`, [0, outputSection.modelData.logical?.y ?? 0])[1]
                            onValueChanged: {
                                const p = page.value(`${outputSection.base}.position`, [outputSection.modelData.logical?.x ?? 0, outputSection.modelData.logical?.y ?? 0]);
                                if (p[1] !== value) page.set(`${outputSection.base}.position`, [p[0], value]);
                            }
                        }
                    }
                }
            }
        }

        ContentSection {
            icon: "mouse"
            title: Translation.tr("Mouse")

            ConfigComboBox {
                Layout.fillWidth: true
                buttonIcon: "trending_up"
                text: Translation.tr("Acceleration")
                fieldWidth: 220
                fixedWidth: true
                model: [
                    { displayName: Translation.tr("Flat (none)"), value: "flat" },
                    { displayName: Translation.tr("Adaptive"), value: "adaptive" },
                ]
                currentValue: page.value("input.type:pointer.accel_profile", page.value("input.*.accel_profile", "adaptive"))
                onSelected: newValue => page.set("input.type:pointer.accel_profile", newValue)
            }
            ConfigSlider {
                Layout.fillWidth: true
                buttonIcon: "speed"
                text: Translation.tr("Speed")
                usePercentTooltip: false
                from: -1; to: 1
                stopIndicatorValues: [0]
                value: page.value("input.type:pointer.pointer_speed", page.value("input.*.pointer_speed", 0))
                onMoved: page.set("input.type:pointer.pointer_speed", Math.round(value * 100) / 100)
            }
        }

        ContentSection {
            id: idleSection
            icon: "timer"
            title: Translation.tr("Idle")

            readonly property var idle: Config.options.hyprland.idle
            readonly property list<var> unitOptions: [
                { displayName: Translation.tr("Seconds"), value: 1 },
                { displayName: Translation.tr("Minutes"), value: 60 },
                { displayName: Translation.tr("Hours"), value: 3600 },
            ]
            function toDisplay(seconds) {
                if (seconds <= 0) return [0, 60];
                if (seconds % 3600 === 0) return [seconds / 3600, 3600];
                if (seconds % 60 === 0) return [seconds / 60, 60];
                return [seconds, 1];
            }

            // One idle time; 0 turns it off.
            component IdleTimerRow: RowLayout {
                id: timerRow
                property string icon
                property string label
                property int seconds: 0
                property int displayValue: idleSection.toDisplay(timerRow.seconds)[0]
                property int displayUnit: idleSection.toDisplay(timerRow.seconds)[1]
                signal edited(int newSeconds)

                Layout.fillWidth: true
                Layout.leftMargin: 8
                spacing: 10
                MaterialSymbol {
                    text: timerRow.icon
                    iconSize: Appearance.font.pixelSize.larger
                    color: Appearance.colors.colOnSecondaryContainer
                }
                StyledText {
                    Layout.fillWidth: true
                    text: timerRow.label
                    color: Appearance.colors.colOnSecondaryContainer
                    elide: Text.ElideRight
                }
                StyledSpinBox {
                    Layout.preferredWidth: 130
                    value: timerRow.displayValue
                    from: 0
                    to: 9999
                    stepSize: 1
                    onValueModified: timerRow.edited(value * timerRow.displayUnit)
                }
                StyledComboBox {
                    Layout.preferredWidth: 140
                    textRole: "displayName"
                    model: idleSection.unitOptions
                    currentIndex: idleSection.unitOptions.findIndex(o => o.value === timerRow.displayUnit)
                    onActivated: index => {
                        timerRow.displayUnit = idleSection.unitOptions[index].value;
                        timerRow.edited(timerRow.displayValue * timerRow.displayUnit);
                    }
                }
            }

            StyledText {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                color: Appearance.colors.colSubtext
                text: Translation.tr("After this long without input; 0 means never. Apps that keep the screen awake (videos, games) and the bar's keep-awake toggle hold these off.")
            }
            IdleTimerRow {
                icon: "lock_clock"
                label: Translation.tr("Lock the screen")
                seconds: idleSection.idle.lock
                onEdited: newSeconds => idleSection.idle.lock = newSeconds
            }
            IdleTimerRow {
                icon: "monitor"
                label: Translation.tr("Turn the screens off")
                seconds: idleSection.idle.screenOff
                onEdited: newSeconds => idleSection.idle.screenOff = newSeconds
            }
            IdleTimerRow {
                icon: "bedtime"
                label: Translation.tr("Sleep")
                seconds: idleSection.idle.suspend
                onEdited: newSeconds => idleSection.idle.suspend = newSeconds
            }
        }

        ContentSection {
            id: appTheming
            icon: "format_paint"
            title: Translation.tr("App theming")

            readonly property var theming: Config.options.appearance.wallpaperTheming
            function toggle(id, on) {
                const list = appTheming.theming.appTemplates.filter(t => t !== id);
                if (on) list.push(id);
                appTheming.theming.appTemplates = list;
                templatesProc.running = true;
            }

            StyledText {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                color: Appearance.colors.colSubtext
                text: Translation.tr("Apps get the wallpaper's colours from templates, rewritten whenever the colours change. A built-in one writes a colours file and includes it from the app's config; one managed by Home Manager is left alone (add the include there yourself).")
            }
            ConfigSwitch {
                buttonIcon: "terminal"
                text: Translation.tr("foot")
                checked: appTheming.theming.appTemplates.includes("foot")
                onCheckedChanged: if (checked !== appTheming.theming.appTemplates.includes("foot")) appTheming.toggle("foot", checked)
            }
            ConfigSwitch {
                buttonIcon: "terminal"
                text: Translation.tr("kitty")
                checked: appTheming.theming.appTemplates.includes("kitty")
                onCheckedChanged: if (checked !== appTheming.theming.appTemplates.includes("kitty")) appTheming.toggle("kitty", checked)
            }
            RowLayout {
                Layout.fillWidth: true
                StyledText {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    color: Appearance.colors.colSubtext
                    text: Translation.tr("Your own templates: ~/.config/hecate/templates.toml")
                }
                RippleButtonWithIcon {
                    buttonRadius: Appearance.rounding.small
                    materialIcon: "edit"
                    mainText: Translation.tr("Edit")
                    onClicked: editTemplatesProc.running = true
                }
                RippleButtonWithIcon {
                    buttonRadius: Appearance.rounding.small
                    materialIcon: "refresh"
                    mainText: Translation.tr("Apply now")
                    enabled: !templatesProc.running
                    onClicked: templatesProc.running = true
                }
            }

            Process {
                id: templatesProc
                command: ["python3", Quickshell.shellPath("scripts/colors/hecate_templates.py")]
                stderr: StdioCollector {
                    onStreamFinished: if (text.trim() !== "") Quickshell.execDetached(["notify-send", "-a", "Shell", Translation.tr("App theming"), text.trim()])
                }
            }
            Process {
                id: editTemplatesProc
                command: ["bash", "-c", `f="${page.dir}/templates.toml"; [ -e "$f" ] || { mkdir -p "${page.dir}/templates"; cat > "$f" <<'TOML'
# App colour templates, rendered with the wallpaper's colours (hecate shell).
# Syntax: {{ colors.<name>.default.<format> }}, e.g. {{ colors.primary.default.hex }}.
# Names: Material roles (primary, on_primary, surface_container_high, ...), term0..term15,
# terminal_background, terminal_foreground, terminal_normal_red, terminal_bright_blue, ...
# Formats: hex, hex_stripped, rgb, rgba, rgb_csv, red, green, blue, alpha.
# Filters: | set_alpha 0.8, | lighten 10, | darken 10, | invert, | grayscale.
#
# [templates.my_app]
# input_path = "~/.config/hecate/templates/my-app.css"
# output_path = "~/.config/my-app/colors.css"   # or a list of paths
# post_hook = "pkill -USR1 my-app"               # optional, runs after writing
TOML
}; xdg-open "$f"`]
            }
        }

        RippleButtonWithIcon {
            Layout.alignment: Qt.AlignRight
            buttonRadius: Appearance.rounding.small
            materialIcon: "restart_alt"
            mainText: Translation.tr("Reset to the Nix config")
            onClicked: resetProc.running = true
        }
    }
}
