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

    function toToml(edits) {
        const sections = {};
        for (const [path, v] of Object.entries(edits)) {
            const parts = path.split(".");
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

    Component.onCompleted: {
        mkdirProc.running = true;
        configProc.running = true;
    }

    Process { id: mkdirProc; command: ["mkdir", "-p", page.dir] }
    Process { id: reloadProc; command: ["hecate", "msg", "reload"] }
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

        RippleButtonWithIcon {
            Layout.alignment: Qt.AlignRight
            buttonRadius: Appearance.rounding.small
            materialIcon: "restart_alt"
            mainText: Translation.tr("Reset to the Nix config")
            onClicked: resetProc.running = true
        }
    }
}
