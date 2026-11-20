pragma ComponentBehavior: Bound
import qs.modules.common
import qs.modules.common.utils
import qs.modules.common.functions
import qs.modules.common.widgets
import qs.modules.common.widgets.haiiro as H
import qs.services
import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

PanelWindow {
    id: root
    visible: false
    color: "transparent"
    WlrLayershell.namespace: "quickshell:regionSelector"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
    exclusionMode: ExclusionMode.Ignore
    anchors {
        left: true
        right: true
        top: true
        bottom: true
    }

    // Modes
    // Ask only puts the region on the clipboard for now; the Claude integration takes it from there.
    // Pick takes one pixel. Ocr, Ask, Translate and Pick are run by RegionSelector (`process`),
    // the others by ScreenshotAction.
    enum SnipAction { Copy, Edit, Search, CharRecognition, Record, RecordWithSound, Ask, Translate, Pick }
    enum SelectionMode { RectCorners, Circle, ScreenTarget }
    enum Phase { Select, Post }
    property var action: RegionSelection.SnipAction.Copy
    property var selectionMode: RegionSelection.SelectionMode.RectCorners
    property var phase: RegionSelection.Phase.Select
    // shot: copy to the clipboard, save to a file, or both
    property bool copyToClipboard: true
    property bool saveToFile: false
    signal dismiss()
    // An action RegionSelector runs: the capture, the region in pixels of the capture.
    signal process(var action, string path, real x, real y, real width, real height)
    // A recording started on this region (logical size).
    signal recordingStarted(real width, real height)

    // Styles
    property string screenshotDir: Directories.screenshotTemp
    // The veil of the Region board: base at 72%.
    property color overlayColor: Qt.alpha(H.HTokens.base, 0.72)
    property color selectionBorderColor: H.HTokens.text
    property color selectionFillColor: Qt.alpha(H.HTokens.text, 0.2)
    property color windowBorderColor: H.HTokens.foam
    property color windowFillColor: Qt.alpha(windowBorderColor, 0.15)
    property color imageBorderColor: H.HTokens.iris
    property color imageFillColor: Qt.alpha(imageBorderColor, 0.15)
    property real targetRegionOpacity: Config.options.regionSelector.targetRegions.opacity
    property bool contentRegionOpacity: Config.options.regionSelector.targetRegions.contentRegionOpacity

    // Vars for indicators
    readonly property var windows: [...HyprlandData.windowList].sort((a, b) => {
        // Sort floating=true windows before others
        if (a.floating === b.floating) return 0;
        return a.floating ? -1 : 1;
    })
    readonly property var layers: HyprlandData.layers
    readonly property real falsePositivePreventionRatio: 0.5

    // Screen & interaction vars
    readonly property var monitor: WM.monitorFor(screen)
    readonly property var monitorGeometry: WM.monitorGeometry(screen)
    readonly property real monitorScale: monitorGeometry.scale
    readonly property real monitorOffsetX: monitorGeometry.x
    readonly property real monitorOffsetY: monitorGeometry.y
    property int activeWorkspaceId: WM.activeWorkspaceForMonitor(root.monitor?.name)?.id ?? 0
    property string screenshotPath: `${root.screenshotDir}/image-${screen.name}.png`
    property real dragStartX: 0
    property real dragStartY: 0
    property real draggingX: 0
    property real draggingY: 0
    property real dragDiffX: 0
    property real dragDiffY: 0
    property bool draggedAway: (dragDiffX !== 0 || dragDiffY !== 0)
    property bool dragging: false
    property list<point> points: []
    property var mouseButton: null
    property var imageRegions: []
    readonly property list<var> windowRegions: RegionFunctions.filterWindowRegionsByLayers(
        root.windows.filter(w => w.workspace.id === root.activeWorkspaceId),
        root.layerRegions
    ).map(window => {
        return {
            at: [window.at[0] - root.monitorOffsetX, window.at[1] - root.monitorOffsetY],
            size: [window.size[0], window.size[1]],
            class: window.class,
            title: window.title,
        }
    })
    readonly property list<var> layerRegions: {
        const layersOfThisMonitor = root.layers[root.monitor?.name]
        const topLayers = layersOfThisMonitor?.levels["2"]
        if (!topLayers) return [];
        const nonBarTopLayers = topLayers
            .filter(layer => !(layer.namespace.includes(":bar") || layer.namespace.includes(":verticalBar") || layer.namespace.includes(":dock")))
            .map(layer => {
            return {
                at: [layer.x, layer.y],
                size: [layer.w, layer.h],
                namespace: layer.namespace,
            }
        })
        const offsetAdjustedLayers = nonBarTopLayers.map(layer => {
            return {
                at: [layer.at[0] - root.monitorOffsetX, layer.at[1] - root.monitorOffsetY],
                size: layer.size,
                namespace: layer.namespace,
            }
        });
        return offsetAdjustedLayers;
    }

    // Config
    property bool isCircleSelection: (root.selectionMode === RegionSelection.SelectionMode.Circle)
    property bool isScreenMode: (root.selectionMode === RegionSelection.SelectionMode.ScreenTarget) && !root.isPick
    property bool isPick: root.action === RegionSelection.SnipAction.Pick
    property bool enableWindowRegions: Config.options.regionSelector.targetRegions.windows && !isCircleSelection && !root.isScreenMode && !root.isPick
    property bool enableLayerRegions: Config.options.regionSelector.targetRegions.layers && !isCircleSelection && !root.isScreenMode && !root.isPick
    property bool enableContentRegions: Config.options.regionSelector.targetRegions.content && !root.isScreenMode && !root.isPick

    // Target
    property real targetedRegionX: -1
    property real targetedRegionY: -1
    property real targetedRegionWidth: 0
    property real targetedRegionHeight: 0
    function targetedRegionValid() {
        return (root.targetedRegionX >= 0 && root.targetedRegionY >= 0)
    }
    function setRegionToTargeted() {
        const padding = Config.options.regionSelector.targetRegions.selectionPadding; // Make borders not cut off n stuff
        root.regionX = root.targetedRegionX - padding;
        root.regionY = root.targetedRegionY - padding;
        root.regionWidth = root.targetedRegionWidth + padding * 2;
        root.regionHeight = root.targetedRegionHeight + padding * 2;
    }

    function selectFullScreen() {
        root.regionX = 0;
        root.regionY = 0;
        root.regionWidth = root.screen.width;
        root.regionHeight = root.screen.height;
    }

    function updateTargetedRegion(x, y) {
        // Image regions
        const clickedRegion = root.isScreenMode ? null : root.imageRegions.find(region => {
            return region.at[0] <= x && x <= region.at[0] + region.size[0] && region.at[1] <= y && y <= region.at[1] + region.size[1];
        });
        if (clickedRegion) {
            root.targetedRegionX = clickedRegion.at[0];
            root.targetedRegionY = clickedRegion.at[1];
            root.targetedRegionWidth = clickedRegion.size[0];
            root.targetedRegionHeight = clickedRegion.size[1];
            return;
        }

        // Layer regions
        const clickedLayer = root.isScreenMode ? null : root.layerRegions.find(region => {
            return region.at[0] <= x && x <= region.at[0] + region.size[0] && region.at[1] <= y && y <= region.at[1] + region.size[1];
        });
        if (clickedLayer) {
            root.targetedRegionX = clickedLayer.at[0];
            root.targetedRegionY = clickedLayer.at[1];
            root.targetedRegionWidth = clickedLayer.size[0];
            root.targetedRegionHeight = clickedLayer.size[1];
            return;
        }

        // Window regions
        const clickedWindow = root.windowRegions.find(region => {
            return region.at[0] <= x && x <= region.at[0] + region.size[0] && region.at[1] <= y && y <= region.at[1] + region.size[1];
        });
        if (clickedWindow) {
            root.targetedRegionX = clickedWindow.at[0];
            root.targetedRegionY = clickedWindow.at[1];
            root.targetedRegionWidth = clickedWindow.size[0];
            root.targetedRegionHeight = clickedWindow.size[1];
            return;
        }

        root.targetedRegionX = -1;
        root.targetedRegionY = -1;
        root.targetedRegionWidth = 0;
        root.targetedRegionHeight = 0;
    }

    property real regionWidth: Math.abs(draggingX - dragStartX)
    property real regionHeight: Math.abs(draggingY - dragStartY)
    property real regionX: Math.min(dragStartX, draggingX)
    property real regionY: Math.min(dragStartY, draggingY)

    // Screenshot stuff
    TempScreenshotProcess {
        id: screenshotProc
        running: true
        screen: root.screen
        screenshotDir: root.screenshotDir
        screenshotPath: root.screenshotPath
        onExited: (exitCode, exitStatus) => {
            if (root.enableContentRegions) imageDetectionProcess.running = true;
            root.preparationDone = !checkRecordingProc.running;
        }
    }
    property bool isRecording: root.action === RegionSelection.SnipAction.Record || root.action === RegionSelection.SnipAction.RecordWithSound
    property bool recordingShouldStop: false
    Process {
        id: checkRecordingProc
        running: isRecording
        command: ["pidof", "wf-recorder"]
        onExited: (exitCode, exitStatus) => {
            root.preparationDone = !screenshotProc.running
            root.recordingShouldStop = (exitCode === 0);
        }
    }
    property bool preparationDone: false
    onPreparationDoneChanged: {
        if (!preparationDone) return;
        if (root.isRecording && root.recordingShouldStop) {
            Quickshell.execDetached([Directories.recordScriptPath]);
            root.dismiss();
            return;
        }
        root.visible = true;
    }

    Connections {
        target: Persistent.states.record
        function onEnableChanged() {
            if (!Persistent.states.record.enable && root.isRecording) {
                root.dismiss();
            }
        }
    }

    Process {
        id: imageDetectionProcess
        command: ["bash", "-c", `${Directories.scriptPath}/images/find-regions-venv.sh ` 
            + `--hyprctl ` 
            + `--image '${StringUtils.shellSingleQuoteEscape(root.screenshotPath)}' ` 
            + `--max-width ${Math.round(root.screen.width * root.falsePositivePreventionRatio)} ` 
            + `--max-height ${Math.round(root.screen.height * root.falsePositivePreventionRatio)} `]
        stdout: StdioCollector {
            id: imageDimensionCollector
            onStreamFinished: {
                imageRegions = RegionFunctions.filterImageRegions(
                    JSON.parse(imageDimensionCollector.text),
                    root.windowRegions
                );
            }
        }
    }

    function getScreenshotAction() {
        switch(root.action) {
            case RegionSelection.SnipAction.Copy:
                return ScreenshotAction.Action.Copy;
            case RegionSelection.SnipAction.Edit:
                return ScreenshotAction.Action.Edit;
            case RegionSelection.SnipAction.Search:
                return ScreenshotAction.Action.Search;
            case RegionSelection.SnipAction.CharRecognition:
                return ScreenshotAction.Action.CharRecognition;
            case RegionSelection.SnipAction.Record:
                return ScreenshotAction.Action.Record;
            case RegionSelection.SnipAction.RecordWithSound:
                return ScreenshotAction.Action.RecordWithSound;
            default:
                console.warn("[Region Selector] Unknown snip action, skipping snip.");
                root.dismiss();
                return;
        }
    }

    readonly property bool runsInSelector: root.action === RegionSelection.SnipAction.CharRecognition
        || root.action === RegionSelection.SnipAction.Ask
        || root.action === RegionSelection.SnipAction.Translate
        || root.action === RegionSelection.SnipAction.Pick
    readonly property string defaultSaveDir: FileUtils.trimFileProtocol(Directories.pictures) + "/Screenshots"

    // Save without copying: the clipboard is left alone.
    function saveOnlyCommand(x, y, width, height, dir) {
        const q = s => `'${StringUtils.shellSingleQuoteEscape(s)}'`;
        const crop = `-crop ${Math.round(width)}x${Math.round(height)}+${Math.round(x)}+${Math.round(y)} +repage`;
        return ["bash", "-c", `mkdir -p ${q(dir)} && f=${q(dir)}/"screenshot-$(date '+%Y-%m-%d_%H.%M.%S').png" && `
            + `if magick 'png:${StringUtils.shellSingleQuoteEscape(root.screenshotPath)}' ${crop} "png:$f.part" && mv "$f.part" "$f"; `
            + `then notify-send -a "Screen Snip" -i image-x-generic "Screenshot saved" "$f"; `
            + `else rm -f "$f.part"; notify-send -a "Screen Snip" -i image-x-generic "Screenshot failed" "could not crop the capture"; fi; `
            + `rm -f ${q(root.screenshotPath)}`];
    }

    // Execution after selection
    function snip() {
        // Validity check
        if (root.regionWidth <= 0 || root.regionHeight <= 0) {
            console.warn("[Region Selector] Invalid region size, skipping snip.");
            root.dismiss();
        }

        // Clamp region to screen bounds
        root.regionX = Math.max(0, Math.min(root.regionX, root.screen.width - root.regionWidth));
        root.regionY = Math.max(0, Math.min(root.regionY, root.screen.height - root.regionHeight));
        root.regionWidth = Math.max(0, Math.min(root.regionWidth, root.screen.width - root.regionX));
        root.regionHeight = Math.max(0, Math.min(root.regionHeight, root.screen.height - root.regionY));

        // Adjust action
        if (root.action === RegionSelection.SnipAction.Copy && root.mouseButton === Qt.RightButton) {
            root.action = RegionSelection.SnipAction.Edit;
        }
        
        const screenshotDir = Config.options.screenSnip.savePath !== "" ? //
            Config.options.screenSnip.savePath : "";
        const px = root.regionX * root.monitorScale;
        const py = root.regionY * root.monitorScale;
        const pw = root.regionWidth * root.monitorScale;
        const ph = root.regionHeight * root.monitorScale;

        // RegionSelector runs these, so it can show what came out of them.
        if (root.runsInSelector) {
            root.process(root.action, root.screenshotPath, px, py, pw, ph);
            root.dismiss();
            return;
        }

        // shot: copy and/or save
        const wantsFile = root.saveToFile && root.action === RegionSelection.SnipAction.Copy;
        const saveDirectory = wantsFile ? (screenshotDir !== "" ? screenshotDir : root.defaultSaveDir) : "";
        if (wantsFile && !root.copyToClipboard) {
            Quickshell.execDetached(root.saveOnlyCommand(px, py, pw, ph, saveDirectory));
            root.dismiss();
            return;
        }

        var screenshotAction = root.getScreenshotAction();
        const command = ScreenshotAction.getCommand(
            px, py, pw, ph, //
            root.screenshotPath, //
            screenshotAction, //
            root.action === RegionSelection.SnipAction.Copy ? saveDirectory : screenshotDir, //
            Config.options.screenRecord.systemAudio, //
            Config.options.screenRecord.microphone
        )
        Quickshell.execDetached(command);
        if (root.action == RegionSelection.SnipAction.Record || root.action == RegionSelection.SnipAction.RecordWithSound) {
            root.recordingStarted(root.regionWidth, root.regionHeight);
            root.phase = RegionSelection.Phase.Post
            root.selectionMode = RegionSelection.SelectionMode.RectCorners
        } else {
            root.dismiss();
        }
    }

    function stepMode(step) {
        const n = RegionModes.modes.length;
        const next = RegionModes.modes[((RegionModes.indexOf(root.action) + step) % n + n) % n];
        root.action = next.action;
        if (root.action === RegionSelection.SnipAction.Record && root.selectionMode === RegionSelection.SelectionMode.Circle)
            root.selectionMode = RegionSelection.SelectionMode.RectCorners;
    }

    function takeNow() {
        if (root.isPick) {
            root.regionX = mouseArea.mouseX;
            root.regionY = mouseArea.mouseY;
            root.regionWidth = 1;
            root.regionHeight = 1;
        } else if (root.isScreenMode) {
            root.selectFullScreen();
        } else if (root.regionWidth < 1 || root.regionHeight < 1) {
            root.updateTargetedRegion(mouseArea.mouseX, mouseArea.mouseY);
            if (root.targetedRegionValid()) root.setRegionToTargeted();
            else root.selectFullScreen();
        }
        root.snip();
    }

    // Only clickable in Selection phase
    mask: Region {
        item: switch(root.phase) {
            case RegionSelection.Phase.Select: return mouseArea;
            case RegionSelection.Phase.Post: return null;
        }
    }

    Image { // For freezing
        anchors.fill: parent
        source: root.preparationDone ? `file://${root.screenshotPath}` : ""
        cache: false
        asynchronous: false
        fillMode: Image.Stretch
        visible: root.phase === RegionSelection.Phase.Select

        focus: root.visible
        Keys.onPressed: (event) => {
            if (root.phase !== RegionSelection.Phase.Select) return;
            switch (event.key) {
            case Qt.Key_Escape: // cancel
                Qt.callLater(root.dismiss);
                break;
            case Qt.Key_Tab: // next mode
                root.stepMode(1);
                break;
            case Qt.Key_Backtab:
                root.stepMode(-1);
                break;
            case Qt.Key_Return: // take what is selected, else the window under the pointer, else the screen
            case Qt.Key_Enter:
                root.takeNow();
                break;
            default:
                return;
            }
            event.accepted = true;
        }
    }

    MouseArea {
        id: mouseArea
        anchors.fill: parent
        cursorShape: Qt.CrossCursor
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        hoverEnabled: true

        // Controls
        onPressed: (mouse) => {
            if (root.isScreenMode || root.isPick) {
                root.mouseButton = mouse.button;
                return;
            }
            root.dragStartX = mouse.x;
            root.dragStartY = mouse.y;
            root.draggingX = mouse.x;
            root.draggingY = mouse.y;
            root.dragging = true;
            root.mouseButton = mouse.button;
        }
        onReleased: (mouse) => {
            if (root.isPick) {
                root.regionX = mouse.x;
                root.regionY = mouse.y;
                root.regionWidth = 1;
                root.regionHeight = 1;
                root.snip();
                return;
            }
            if (root.isScreenMode) {
                root.selectFullScreen();
                root.snip();
                return;
            }
            // Detect if it was a click -> Try to select targeted region
            if (root.draggingX === root.dragStartX && root.draggingY === root.dragStartY) {
                if (root.targetedRegionValid()) {
                    root.setRegionToTargeted();
                }
            }
            // Circle dragging?
            else if (root.selectionMode === RegionSelection.SelectionMode.Circle) {
                const padding = Config.options.regionSelector.circle.padding + Config.options.regionSelector.circle.strokeWidth / 2;
                const dragPoints = (root.points.length > 0) ? root.points : [{ x: mouseArea.mouseX, y: mouseArea.mouseY }];
                const maxX = Math.max(...dragPoints.map(p => p.x));
                const minX = Math.min(...dragPoints.map(p => p.x));
                const maxY = Math.max(...dragPoints.map(p => p.y));
                const minY = Math.min(...dragPoints.map(p => p.y));
                root.regionX = minX - padding;
                root.regionY = minY - padding;
                root.regionWidth = maxX - minX + padding * 2;
                root.regionHeight = maxY - minY + padding * 2;
            }
            root.snip();
        }
        onPositionChanged: (mouse) => {
            root.updateTargetedRegion(mouse.x, mouse.y);
            if (!root.dragging) return;
            root.draggingX = mouse.x;
            root.draggingY = mouse.y;
            root.dragDiffX = mouse.x - root.dragStartX;
            root.dragDiffY = mouse.y - root.dragStartY;
            root.points.push({ x: mouse.x, y: mouse.y });
        }
        
        Loader {
            z: 2
            anchors.fill: parent
            active: root.selectionMode !== RegionSelection.SelectionMode.Circle
            sourceComponent: RectCornersSelectionDetails {
                regionX: root.regionX
                regionY: root.regionY
                regionWidth: root.regionWidth
                regionHeight: root.regionHeight
                mouseX: mouseArea.mouseX
                mouseY: mouseArea.mouseY
                color: root.selectionBorderColor
                overlayColor: root.overlayColor
                showAimLines: root.isScreenMode ? false : Config.options.regionSelector.rect.showAimLines
                breathingBorderOnly: root.phase === RegionSelection.Phase.Post
            }
        }

        Loader {
            z: 2
            anchors.fill: parent
            active: root.selectionMode === RegionSelection.SelectionMode.Circle
            sourceComponent: CircleSelectionDetails {
                color: root.selectionBorderColor
                overlayColor: root.overlayColor
                points: root.points
            }
        }

        // The thing to the bottom-right with an icon
        CursorGuide {
            z: 9999
            visible: root.phase === RegionSelection.Phase.Select
            x: root.dragging ? root.regionX + root.regionWidth : mouseArea.mouseX
            y: root.dragging ? root.regionY + root.regionHeight : mouseArea.mouseY
            action: root.action
            selectionMode: root.selectionMode
        }

        // Window regions
        Repeater {
            model: ScriptModel {
                values: {
                    if (root.phase === RegionSelection.Phase.Select && root.enableWindowRegions) {
                        return root.windowRegions
                    } else {
                        return []
                    }
                }
            }
            delegate: TargetRegion {
                z: 2
                required property var modelData
                clientDimensions: modelData
                showIcon: true
                targeted: !root.draggedAway && //
                    (root.targetedRegionX === modelData.at[0]  //
                    && root.targetedRegionY === modelData.at[1] //
                    && root.targetedRegionWidth === modelData.size[0] //
                    && root.targetedRegionHeight === modelData.size[1])

                opacity: root.draggedAway ? 0 : (targeted ? 1 : root.targetRegionOpacity)
                borderColor: root.windowBorderColor
                fillColor: targeted ? root.windowFillColor : "transparent"
                text: modelData.title ? `${modelData.class}: ${modelData.title}` : `${modelData.class}`
            }
        }

        // Layer regions
        Repeater {
            model: ScriptModel {
                values: {
                    if (root.phase === RegionSelection.Phase.Select && root.enableLayerRegions) {
                        return root.layerRegions
                    } else {
                        return []
                    }
                }
            }
            delegate: TargetRegion {
                z: 3
                required property var modelData
                clientDimensions: modelData
                targeted: !root.draggedAway &&
                    (root.targetedRegionX === modelData.at[0] 
                    && root.targetedRegionY === modelData.at[1]
                    && root.targetedRegionWidth === modelData.size[0]
                    && root.targetedRegionHeight === modelData.size[1])

                opacity: root.draggedAway ? 0 : (targeted ? 1 : root.targetRegionOpacity)
                borderColor: root.windowBorderColor
                fillColor: targeted ? root.windowFillColor : "transparent"
                text: `${modelData.namespace}`
            }
        }

        // Content regions
        Repeater {
            model: ScriptModel {
                values: {
                    if (root.phase === RegionSelection.Phase.Select && root.enableContentRegions) {
                        return root.imageRegions
                    } else {
                        return []
                    }
                }
            }
            delegate: TargetRegion {
                z: 4
                required property var modelData
                clientDimensions: modelData
                targeted: !root.draggedAway &&
                    (root.targetedRegionX === modelData.at[0] 
                    && root.targetedRegionY === modelData.at[1]
                    && root.targetedRegionWidth === modelData.size[0]
                    && root.targetedRegionHeight === modelData.size[1])

                opacity: root.draggedAway ? 0 : (targeted ? 1 : root.contentRegionOpacity)
                borderColor: root.imageBorderColor
                fillColor: targeted ? root.imageFillColor : "transparent"
                text: "picture"
            }
        }

        // The mode bar
        H.PixelTransition {
            id: barTransition
            z: 10
            shown: root.visible && root.phase === RegionSelection.Phase.Select
            anchors {
                horizontalCenter: parent.horizontalCenter
                bottom: parent.bottom
                bottomMargin: 40
            }
            width: modeBar.implicitWidth
            height: modeBar.implicitHeight

            ModeBar {
                id: modeBar
                action: root.action
                selectionMode: root.selectionMode
                copyToClipboard: root.copyToClipboard
                saveToFile: root.saveToFile
                onActionPicked: action => {
                    root.action = action;
                    if (action === RegionSelection.SnipAction.Record && root.selectionMode === RegionSelection.SelectionMode.Circle)
                        root.selectionMode = RegionSelection.SelectionMode.RectCorners;
                }
                onSelectionModePicked: mode => root.selectionMode = mode
                // at least one of the two stays on
                onCopyToggled: if (!root.copyToClipboard || root.saveToFile) root.copyToClipboard = !root.copyToClipboard
                onSaveToggled: if (!root.saveToFile || root.copyToClipboard) root.saveToFile = !root.saveToFile
            }
        }
    }
}
