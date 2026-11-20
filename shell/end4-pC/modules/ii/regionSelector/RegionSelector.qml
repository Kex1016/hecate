pragma ComponentBehavior: Bound
import qs
import qs.modules.common
import qs.modules.common.functions
import qs.modules.common.models.gCloud
import qs.services
import qs.services.claude
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland

Scope {
    id: root

    function dismiss() {
        GlobalStates.regionSelectorOpen = false
    }

    property var action: RegionSelection.SnipAction.Copy
    property var selectionMode: RegionSelection.SelectionMode.RectCorners
    // shot: where it goes. Saving is on from the start when a save folder is set.
    property bool copyToClipboard: true
    property bool saveToFile: Config.options.screenSnip.savePath !== ""

    readonly property var focusedScreen: Quickshell.screens.find(s => s.name === WM.focusedMonitor?.name) ?? Quickshell.screens[0]

    Variants {
        model: Quickshell.screens
        delegate: Loader {
            id: regionSelectorLoader
            required property var modelData
            active: GlobalStates.regionSelectorOpen

            sourceComponent: RegionSelection {
                screen: regionSelectorLoader.modelData
                onDismiss: root.dismiss()
                action: root.action
                selectionMode: root.selectionMode
                copyToClipboard: root.copyToClipboard
                saveToFile: root.saveToFile
                onProcess: (action, path, x, y, width, height) => root.run(action, path, x, y, width, height)
                onRecordingStarted: (width, height) => recordIndicator.size = `${Math.round(width)}×${Math.round(height)}`
            }
        }
    }

    // ── what the tools do with a region ──────────────────────────────────────────────────────

    function q(text) {
        return `'${StringUtils.shellSingleQuoteEscape(text)}'`;
    }
    function cropCommand(path, x, y, width, height) {
        return `magick 'png:${StringUtils.shellSingleQuoteEscape(path)}' -crop ${Math.round(width)}x${Math.round(height)}+${Math.round(x)}+${Math.round(y)} +repage`;
    }
    function copyText(text) {
        Quickshell.execDetached(["bash", "-c", "printf '%s' \"$1\" | wl-copy", "_", text]);
    }

    property var job: null

    function run(action, path, x, y, width, height) {
        root.job = { action, path };
        const crop = root.cropCommand(path, x, y, width, height);
        const cleanup = `rm -f ${root.q(path)}`;
        switch (action) {
        case RegionSelection.SnipAction.CharRecognition:
        case RegionSelection.SnipAction.Translate:
            // every installed language at once, like the old text recognition did
            ocrProc.command = ["bash", "-c", `set -o pipefail; ${crop} png:- | tesseract stdin stdout -l "$(tesseract --list-langs | awk 'NR>1 && $1 != "osd" {print $1}' | tr '\\n' '+' | sed 's/+$//')" 2>/dev/null; ${cleanup}`];
            ocrProc.running = true;
            break;
        case RegionSelection.SnipAction.Pick:
            pickProc.command = ["bash", "-c", `${crop} -format '#%[hex:u.p{0,0}]' info: | cut -c1-7; ${cleanup}`];
            pickProc.running = true;
            break;
        case RegionSelection.SnipAction.Ask:
            if (!(Config.options.claude?.enable ?? false)) {
                Quickshell.execDetached(["rm", "-f", path]);
                toast.show("claude: ~/ask", "gold", [
                    { text: "Claude is off." },
                    { text: "Switch it on under settings, services.", role: "caption", ink: "c1" }
                ]);
                break;
            }
            // The ask panel (ClaudeAsk) takes the region as a PNG and opens with only it on.
            askProc.region = { path: ClaudeAsk.regionPath, width: width, height: height };
            askProc.command = ["bash", "-c", `mkdir -p "$(dirname ${root.q(ClaudeAsk.regionPath)})" && ${crop} ${root.q("png:" + ClaudeAsk.regionPath)}; status=$?; ${cleanup}; exit $status`];
            askProc.running = true;
            break;
        }
    }

    Process {
        id: ocrProc
        stdout: StdioCollector {
            id: ocrOut
            onStreamFinished: {
                const text = ocrOut.text.trim();
                if (text === "") {
                    toast.show("ocr: nothing", "gold", [
                        { text: "No text in the region." },
                        { text: "Is tesseract installed?", role: "caption", ink: "c1" }
                    ]);
                } else if (root.job?.action === RegionSelection.SnipAction.Translate) {
                    root.translate(text);
                } else {
                    root.copyText(text);
                    toast.show("ocr: copied", "foam", [{ text: text }]);
                }
            }
        }
    }

    Process {
        id: pickProc
        stdout: StdioCollector {
            id: pickOut
            onStreamFinished: {
                const hex = pickOut.text.trim();
                if (!/^#[0-9a-fA-F]{6}$/.test(hex)) {
                    toast.show("pick: failed", "love", [{ text: "Could not read that pixel." }]);
                    return;
                }
                root.copyText(hex);
                toast.show("pick: copied", "foam", [{ text: hex }], hex);
            }
        }
    }

    Process {
        id: askProc
        property var region: null
        onExited: code => {
            if (code === 0)
                ClaudeAsk.askAboutRegion(region.path, region.width, region.height);
            else
                toast.show("claude: ~/ask", "love", [{ text: "Could not take the region." }]);
        }
    }

    property string translating: ""
    function translate(text) {
        root.translating = text;
        cloudTranslate.translateStrings([text]);
    }
    GCloudTranslate {
        id: cloudTranslate
        onError: message => toast.show("tr: failed", "love", [
            { text: message || Translation.tr("Set your Google Cloud service account key") }
        ])
        onFinished: {
            const result = outputData.translations?.[0];
            const from = result?.detectedLanguageCode ?? "??";
            toast.show(`tr: ${from} » ${Translation.languageCode}`, "pine", [
                { text: root.translating, ink: "c1" },
                { text: result?.translatedText ?? "" }
            ]);
        }
    }

    RegionToast {
        id: toast
        screen: root.focusedScreen
        topMargin: recordIndicator.heightWithGap > 0 ? recordIndicator.topMargin + recordIndicator.heightWithGap : 56
    }
    RecordIndicator {
        id: recordIndicator
        screen: root.focusedScreen
    }

    function screenshot() {
        if (Persistent.states.record.enable) {
            const saveDir = Config.options.screenSnip.savePath !== "" ? Config.options.screenSnip.savePath : "";
            if (saveDir !== "") {
                const cmd = `mkdir -p '${saveDir}' && filePath="${saveDir}/screenshot-$(date '+%Y-%m-%d_%H.%M.%S').png" && grim -g "$(slurp)" "$filePath" && cat "$filePath" | wl-copy && notify-send "Screenshot Saved" "Saved to $filePath" -a "Screen Snip" -i "image-x-generic"`;
                Quickshell.execDetached(["bash", "-c", cmd]);
            } else {
                const cmd = `grim -g "$(slurp)" - | wl-copy && notify-send "Screenshot Copied" "Copied to clipboard" -a "Screen Snip" -i "image-x-generic"`;
                Quickshell.execDetached(["bash", "-c", cmd]);
            }
            return;
        }
        root.action = RegionSelection.SnipAction.Copy
        root.selectionMode = RegionSelection.SelectionMode.RectCorners
        GlobalStates.regionSelectorOpen = true
    }

    function ask() {
        root.action = RegionSelection.SnipAction.Ask
        root.selectionMode = RegionSelection.SelectionMode.RectCorners
        GlobalStates.regionSelectorOpen = true
    }

    function translateRegion() {
        root.action = RegionSelection.SnipAction.Translate
        root.selectionMode = RegionSelection.SelectionMode.RectCorners
        GlobalStates.regionSelectorOpen = true
    }

    function pick() {
        root.action = RegionSelection.SnipAction.Pick
        root.selectionMode = RegionSelection.SelectionMode.RectCorners
        GlobalStates.regionSelectorOpen = true
    }

    function search() {
        root.action = RegionSelection.SnipAction.Search
        if (Config.options.search.imageSearch.useCircleSelection) {
            root.selectionMode = RegionSelection.SelectionMode.Circle
        } else {
            root.selectionMode = RegionSelection.SelectionMode.RectCorners
        }
        GlobalStates.regionSelectorOpen = true
    }

    function ocr() {
        root.action = RegionSelection.SnipAction.CharRecognition
        root.selectionMode = RegionSelection.SelectionMode.RectCorners
        GlobalStates.regionSelectorOpen = true
    }

    function record() {
        if (Persistent.states.record.enable) {
            Quickshell.execDetached([Directories.recordScriptPath]);
            return;
        }
        root.action = RegionSelection.SnipAction.Record
        root.selectionMode = RegionSelection.SelectionMode.RectCorners
        GlobalStates.regionSelectorOpen = true
    }

    function recordWithSound() {
        if (Persistent.states.record.enable) {
            Quickshell.execDetached([Directories.recordScriptPath]);
            return;
        }
        Config.options.screenRecord.systemAudio = true
        root.action = RegionSelection.SnipAction.Record
        root.selectionMode = RegionSelection.SelectionMode.RectCorners
        GlobalStates.regionSelectorOpen = true
    }

    IpcHandler {
        target: "region"

        function screenshot() {
            root.screenshot()
        }
        function search() {
            root.search()
        }
        function ocr() {
            root.ocr()
        }
        function ask() {
            root.ask()
        }
        function translate() {
            root.translateRegion()
        }
        function pick() {
            root.pick()
        }
        function record() {
            root.record()
        }
        function recordWithSound() {
            root.recordWithSound()
        }
    }

    CompositorGlobalShortcut {
        name: "regionScreenshot"
        description: "Takes a screenshot of the selected region"
        onPressed: root.screenshot()
    }
    CompositorGlobalShortcut {
        name: "regionSearch"
        description: "Searches the selected region"
        onPressed: root.search()
    }
    CompositorGlobalShortcut {
        name: "regionOcr"
        description: "Recognizes text in the selected region"
        onPressed: root.ocr()
    }
    CompositorGlobalShortcut {
        name: "regionAsk"
        description: "Asks Claude about the selected region"
        onPressed: root.ask()
    }
    CompositorGlobalShortcut {
        name: "regionTranslate"
        description: "Translates the text in the selected region"
        onPressed: root.translateRegion()
    }
    CompositorGlobalShortcut {
        name: "regionPick"
        description: "Copies the colour of a pixel"
        onPressed: root.pick()
    }
    CompositorGlobalShortcut {
        name: "regionRecord"
        description: "Records the selected region"
        onPressed: root.record()
    }
    CompositorGlobalShortcut {
        name: "regionRecordWithSound"
        description: "Records the selected region with sound"
        onPressed: root.recordWithSound()
    }
}