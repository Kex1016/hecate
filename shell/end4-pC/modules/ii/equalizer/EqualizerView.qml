pragma ComponentBehavior: Bound
import qs
import qs.modules.common
import qs.modules.common.widgets.haiiro
import qs.services
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io

// A 10-band live equalizer in the haiiro window `hecate: ~/media/eq`. Talks to
// EasyEffects through scripts/eq/equalizer.sh (all of that plumbing -
// refresh/setBand/saveChanges/applyPreset/eqGetProc - is unchanged, only the
// window built on top of it is new): preset chips, ten bars you drag, the
// preamp, and a status line.
HWindow {
    id: root

    signal closeRequested()

    // b1..b10 gains in dB, mirrors eq_state.json written by equalizer.sh
    property var bands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    property string presetName: "Flat"
    // Which named custom preset (if any) the CURRENT editing session
    // started from - separate from presetName, which flips to generic
    // "Custom" the instant you touch a slider (see setBand()). Lets the
    // Save rail offer a one-tap "Update '<name>'" that writes back into
    // that same custom-preset entry, instead of only ever being able to
    // commit into whichever EasyEffects preset happens to be active. Set
    // in applyCustomPreset(), cleared in applyPreset() (switching to a
    // built-in curve), and naturally resets to "" on next popup open
    // either way since this whole Item gets recreated then.
    property string editingCustomPresetName: ""
    property bool pending: false
    readonly property var bandLabels: ["32", "63", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"]
    readonly property real bandRange: 12 // -12dB .. +12dB, matches equalizer.sh clamp expectations
    // Master gain on top of the 10 bands (maps to the equalizer block's
    // own output-gain) - independent of preset/curve, so it survives
    // switching presets rather than resetting with them.
    property real preamp: 0
    readonly property real preampRange: 12

    // Mirrors equalizer.sh's save_preset() calls exactly, so tapping a
    // preset chip moves the blobs immediately instead of waiting on a
    // shell round-trip to read the state file back.
    readonly property var presetValues: ({
        "Flat":    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        "Bass":    [5, 7, 5, 2, 1, 0, 0, 0, 1, 2],
        "Treble":  [-2, -1, 0, 1, 2, 3, 4, 5, 6, 6],
        "Vocal":   [-2, -1, 1, 3, 5, 5, 4, 2, 1, 0],
        "Pop":     [2, 4, 2, 0, 1, 2, 4, 2, 1, 2],
        "Rock":    [5, 4, 2, -1, -2, -1, 2, 4, 5, 6],
        "Jazz":    [3, 3, 1, 1, 1, 1, 2, 1, 2, 3],
        "Classic": [0, 1, 2, 2, 2, 2, 1, 2, 3, 4]
    })
    // Auto EQ (genre-follow) now lives in EqualizerAutoService so it keeps
    // running whether or not this view/popup is open - see that file for
    // why. root.autoEnabled below just mirrors the service's state for
    // display; toggleAuto()/maybeLookupGenre() below delegate to it too.
    readonly property bool autoEnabled: EqualizerAutoService.autoEnabled

    // Mirrors equalizer.sh's custom preset store (get_custom/save_custom/
    // delete_custom) - already implemented backend-side, just not exposed
    // in the UI before.
    property string lastfmKey: ""
    // Set when equalizer.sh refused to touch a preset that's never been
    // explicitly saved in EasyEffects (see get_needs_save) - applying an
    // EQ change in that state would otherwise silently wipe any live,
    // unsaved effects (Crystalizer, compressor, etc.) you have running.
    property bool needsManualSave: false
    // name -> [b1..b10], kept in sync with custom_presets.json
    property var customPresets: ({})
    // The one line of text the window can ask for: "save" a curve under a
    // name, or paste the last.fm "key" Auto needs. Empty means no prompt.
    property string promptMode: ""
    onPromptModeChanged: {
        if (root.promptMode.length > 0) {
            prompt.text = ""
            prompt.forceInputFocus()
        }
    }

    // Which EasyEffects preset (i.e. effect chain - compressor, limiter,
    // deesser, etc) the equalizer curve gets merged into. Was previously
    // hardcoded to a guess ("output") with no way to change it from here -
    // see equalizer.sh's ACTIVE_PRESET_FILE comment for why that guess is
    // very likely wrong for any given user.
    property string activePreset: "output"
    property var availablePresets: []

    function refresh() {
        eqGetProc.running = false
        eqGetProc.running = true
    }

    function setBand(index, value) {
        root.bands[index] = value
        root.bandsChanged()
        root.presetName = "Custom"
        // Keep EqualizerAutoService's mirror of the active preset in sync -
        // otherwise a later genre lookup that resolves back to whatever
        // preset was active before this edit sees no change and skips
        // re-applying it, even though the real active preset is now Custom.
        EqualizerAutoService.currentPresetName = "Custom"
        root.pending = true
        Quickshell.execDetached(["bash", Directories.eqScriptPath, Directories.eqStateDir, "set_band", String(index + 1), String(Math.round(value))])
        liveApplyTimer.restart()
    }

    // Doesn't touch presetName - preamp is a master gain layered on top
    // of whichever curve is active, not part of the curve's identity, so
    // adjusting it shouldn't make the preset display flip to "Custom".
    function setPreamp(value) {
        root.preamp = value
        root.pending = true
        Quickshell.execDetached(["bash", Directories.eqScriptPath, Directories.eqStateDir, "set_preamp", String(Math.round(value))])
        liveApplyTimer.restart()
    }

    // The real commit - writes into the actual active preset's own file
    // and clears the unsaved indicator. Only ever called by the user
    // explicitly hitting Save now (see the pill below) - dragging a
    // slider no longer reaches this function at all.
    function saveChanges() {
        Quickshell.execDetached(["bash", Directories.eqScriptPath, Directories.eqStateDir, "save"])
        root.pending = false
        needsSaveRecheckTimer.restart()
    }

    // Debounced live-audio preview while dragging - merges the current
    // slider state into a scratch preset and reloads THAT, so you hear
    // the change immediately without touching the real active preset's
    // file. root.pending deliberately stays true after this: it's audible
    // now, but still unsaved, and the Save pill should keep reflecting that
    // until you actually hit it.
    function previewChanges() {
        Quickshell.execDetached(["bash", Directories.eqScriptPath, Directories.eqStateDir, "preview"])
    }

    // Fires shortly after band/preamp sliders go quiet, so you hear the
    // result without needing to hit Save yourself. Restarted (not just
    // started) on every onMoved, so continuous dragging doesn't spam
    // reloads - it only actually previews once you settle on a value.
    // Deliberately not fired on every single onMoved tick: apply_eq()
    // reloads the whole EasyEffects pipeline each time, which can pop/glitch
    // audio if done many times a second while actively dragging.
    Timer {
        id: liveApplyTimer
        interval: 120
        repeat: false
        onTriggered: root.previewChanges()
    }

    function applyPreset(name) {
        // Update the blobs instantly from the known preset values...
        const vals = root.presetValues[name]
        if (vals) root.bands = vals.slice()
        root.presetName = name
        // A built-in curve, not a named custom preset - nothing for the
        // "Update" shortcut to write back into.
        root.editingCustomPresetName = ""
        // See setBand() above - keeps Auto's stale-preset check honest.
        EqualizerAutoService.currentPresetName = name
        root.pending = false
        // ...while the backend writes + loads the matching EasyEffects preset.
        Quickshell.execDetached(["bash", Directories.eqScriptPath, Directories.eqStateDir, "preset", name])
        needsSaveRecheckTimer.restart()
    }

    function refreshNeedsSave() {
        eqGetNeedsSaveProc.running = false
        eqGetNeedsSaveProc.running = true
    }

    function refreshLastfmKey() {
        eqGetLastfmKeyProc.running = false
        eqGetLastfmKeyProc.running = true
    }

    function saveLastfmKey(key) {
        const trimmed = key.trim()
        root.lastfmKey = trimmed
        Quickshell.execDetached(["bash", Directories.eqScriptPath, Directories.eqStateDir, "set_lastfm_key", trimmed])
        root.promptMode = ""
        // A key just got set (or cleared) - re-run the current song's
        // lookup instead of waiting for the next track change.
        EqualizerAutoService.lastLookupKey = ""
        EqualizerAutoService.maybeLookupGenre()
    }

    function toggleAuto() {
        const goingOn = !EqualizerAutoService.autoEnabled
        EqualizerAutoService.toggleAuto()
        // Auto can't do anything without a key to look genres up with -
        // open the paste field right away instead of letting it silently
        // no-op every track change.
        if (goingOn && root.lastfmKey.length === 0) root.promptMode = "key"
    }

    function refreshCustomPresets() {
        eqGetCustomProc.running = false
        eqGetCustomProc.running = true
    }

    // Same instant-update-then-backend-catches-up pattern as applyPreset(),
    // just routed at "preset" NAME, which equalizer.sh falls through to
    // apply_custom_preset() for anything that isn't one of the 8 built-ins.
    function applyCustomPreset(name) {
        const vals = root.customPresets[name]
        if (vals) root.bands = vals.slice()
        root.presetName = name
        // This IS a named custom preset - remember it so the Save rail can
        // offer "Update '<name>'" if you go on to edit it.
        root.editingCustomPresetName = name
        // See setBand() above - keeps Auto's stale-preset check honest.
        EqualizerAutoService.currentPresetName = name
        root.pending = false
        Quickshell.execDetached(["bash", Directories.eqScriptPath, Directories.eqStateDir, "preset", name])
        needsSaveRecheckTimer.restart()
    }

    function saveCustomPreset(name) {
        const trimmed = name.trim()
        if (!trimmed) return
        const args = ["bash", Directories.eqScriptPath, Directories.eqStateDir, "save_custom", trimmed]
        for (const v of root.bands) args.push(String(Math.round(v)))
        Quickshell.execDetached(args)
        const updated = Object.assign({}, root.customPresets)
        updated[trimmed] = root.bands.slice()
        root.customPresets = updated
        root.presetName = trimmed
        // Whether this came from the "Update '<name>'" shortcut or typing
        // a fresh name into "save as", that name is now what you're
        // editing - and save_custom already committed everything (state
        // file + the real active EasyEffects preset) via apply_eq on the
        // backend side, so there's nothing left unsaved.
        root.editingCustomPresetName = trimmed
        root.pending = false
        // See setBand() above - keeps Auto's stale-preset check honest.
        EqualizerAutoService.currentPresetName = trimmed
        root.promptMode = ""
        needsSaveRecheckTimer.restart()
    }

    function deleteCustomPreset(name) {
        Quickshell.execDetached(["bash", Directories.eqScriptPath, Directories.eqStateDir, "delete_custom", name])
        const updated = Object.assign({}, root.customPresets)
        delete updated[name]
        root.customPresets = updated
        if (root.presetName === name) {
            root.presetName = "Custom"
            root.editingCustomPresetName = ""
            // See setBand() above - keeps Auto's stale-preset check honest.
            EqualizerAutoService.currentPresetName = "Custom"
        }
    }

    function refreshActivePreset() {
        eqGetActivePresetProc.running = false
        eqGetActivePresetProc.running = true
    }

    function refreshAvailablePresets() {
        eqListPresetsProc.running = false
        eqListPresetsProc.running = true
    }

    // Only names EasyEffects already has: switching to a new one would
    // discard whatever is live and unsaved there.
    function setActivePreset(name) {
        const trimmed = name.trim()
        if (!trimmed || root.availablePresets.indexOf(trimmed) === -1) return
        root.activePreset = trimmed
        // The backend re-runs apply_eq() itself as part of set_active_preset
        // (see equalizer.sh), which may find this preset was never manually
        // saved in EasyEffects either - recheck rather than assume it's fine.
        Quickshell.execDetached(["bash", Directories.eqScriptPath, Directories.eqStateDir, "set_active_preset", trimmed])
        needsSaveRecheckTimer.restart()
    }

    function cycleActivePreset() {
        const list = root.availablePresets
        if (list.length < 2) return
        const i = list.indexOf(root.activePreset)
        root.setActivePreset(list[(i + 1) % list.length])
    }

    // A short one-shot delay before polling get_needs_save after any
    // action that runs apply_eq() on the backend (execDetached is
    // fire-and-forget, so there's no direct completion signal to hook -
    // this just gives the detached bash process a moment to finish its
    // file write before we ask about the result).
    Timer {
        id: needsSaveRecheckTimer
        interval: 350
        onTriggered: root.refreshNeedsSave()
    }

    // EqualizerAutoService runs in the background independent of this view,
    // so a genre-triggered preset switch can happen while the popup is
    // already open (or was open before the track changed). Without this,
    // the sliders/preset chip only ever reflected whatever was on disk at
    // Component.onCompleted - i.e. Auto looked like it "worked" only if you
    // closed and reopened the popup. Mirror the service's applied preset
    // straight into the view's own display, the same way applyPreset()
    // already does for a manual tap - all of Auto's targets (Rock, Classic,
    // Jazz, Bass, Vocal, Pop) are built-ins with known values, so no disk
    // read (and no race with the backend's own async write) is needed.
    Connections {
        target: EqualizerAutoService
        function onCurrentPresetNameChanged() {
            const name = EqualizerAutoService.currentPresetName
            const vals = root.presetValues[name]
            if (vals) {
                root.bands = vals.slice()
                root.presetName = name
                root.pending = false
                needsSaveRecheckTimer.restart()
            }
        }
    }

    Component.onCompleted: {
        root.refresh()
        root.refreshLastfmKey()
        root.refreshCustomPresets()
        root.refreshNeedsSave()
        root.refreshActivePreset()
        // Needed up front (not just when the picker's opened) so the
        // needsManualSave banner below can tell "you have presets, just
        // pick one" apart from "you don't have any yet, make one."
        root.refreshAvailablePresets()
    }

    // Watches the shared state so the click-outside and Escape paths in
    // EqualizerPopup.qml, which only flip GlobalStates.equalizerOpen, are
    // caught too.
    Connections {
        target: GlobalStates
        function onEqualizerOpenChanged() {
            if (!GlobalStates.equalizerOpen) {
                // Closing without an explicit Save should discard whatever's
                // been previewing, not commit it - that's the whole point of
                // splitting preview from Save (see setBand/setPreamp above).
                // Stop any debounce still waiting to fire (no point previewing
                // a value we're about to throw away) and tell the backend to
                // revert: pulls the sliders' next-open values back out of the
                // real saved preset and reloads that live, discarding the
                // scratch preview - see revert_preview() in equalizer.sh.
                if (root.pending) {
                    liveApplyTimer.stop()
                    Quickshell.execDetached(["bash", Directories.eqScriptPath, Directories.eqStateDir, "revert_preview"])
                }
            }
        }
    }

    Process {
        id: eqGetProc
        command: ["bash", Directories.eqScriptPath, Directories.eqStateDir, "get"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const data = JSON.parse(text)
                    root.bands = [
                    Number(data.b1), Number(data.b2), Number(data.b3), Number(data.b4), Number(data.b5),
                    Number(data.b6), Number(data.b7), Number(data.b8), Number(data.b9), Number(data.b10)
                    ]
                    root.presetName = data.preset ?? "Custom"
                    // Reopening the popup recreates this whole Item, so
                    // editingCustomPresetName always starts blank - if the
                    // preset that was active when you last closed happens
                    // to be one of your named custom presets, restore the
                    // tracking so "Update '<name>'" can show without you
                    // having to re-click it in the list first. customPresets
                    // may not have loaded yet (separate async request) -
                    // the matching check in eqGetCustomProc below covers
                    // that ordering too.
                    if (Object.prototype.hasOwnProperty.call(root.customPresets, root.presetName))
                        root.editingCustomPresetName = root.presetName
                    // Safety net alongside the explicit syncs in setBand()/
                    // applyPreset()/etc. - if anything else ever writes the
                    // state file, Auto's mirror still gets corrected here.
                    EqualizerAutoService.currentPresetName = root.presetName
                    root.pending = !!data.pending
                    root.preamp = Number(data.preamp) || 0
                } catch (e) {
                    // Leave previous values if the state file isn't ready yet
                }
            }
        }
    }

    Process {
        id: eqGetLastfmKeyProc
        command: ["bash", Directories.eqScriptPath, Directories.eqStateDir, "get_lastfm_key"]
        stdout: StdioCollector {
            onStreamFinished: {
                root.lastfmKey = text.trim()
            }
        }
    }

    Process {
        id: eqGetNeedsSaveProc
        command: ["bash", Directories.eqScriptPath, Directories.eqStateDir, "get_needs_save"]
        stdout: StdioCollector {
            onStreamFinished: {
                const t = text.trim()
                root.needsManualSave = (t === "true" || t === "1")
            }
        }
    }

    Process {
        id: eqGetCustomProc
        command: ["bash", Directories.eqScriptPath, Directories.eqStateDir, "get_custom"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const data = JSON.parse(text)
                    const parsed = {}
                    for (const name in data) {
                        const b = data[name]
                        parsed[name] = [b.b1, b.b2, b.b3, b.b4, b.b5, b.b6, b.b7, b.b8, b.b9, b.b10].map(Number)
                    }
                    root.customPresets = parsed
                    // See the matching comment in eqGetProc above - covers
                    // the case where this request finishes AFTER the state
                    // load already set presetName.
                    if (Object.prototype.hasOwnProperty.call(parsed, root.presetName))
                        root.editingCustomPresetName = root.presetName
                } catch (e) {
                    // Leave previous values if custom_presets.json isn't ready yet
                }
            }
        }
    }

    Process {
        id: eqGetActivePresetProc
        command: ["bash", Directories.eqScriptPath, Directories.eqStateDir, "get_active_preset"]
        stdout: StdioCollector {
            onStreamFinished: {
                const trimmed = text.trim()
                root.activePreset = trimmed.length > 0 ? trimmed : "output"
            }
        }
    }

    Process {
        id: eqListPresetsProc
        command: ["bash", Directories.eqScriptPath, Directories.eqStateDir, "list_presets"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.availablePresets = JSON.parse(text)
                } catch (e) {
                    root.availablePresets = []
                }
            }
        }
    }

    // ---- the window ----
    label: "hecate: ~/media/eq"
    hints: [{ key: "drag", text: "set a band" }, { key: "esc", text: "close" }]
    implicitWidth: 560
    focus: true
    Keys.onEscapePressed: root.closeRequested()

    // built-in curves in the order of the script, then your saved ones
    readonly property var presetNames: Object.keys(root.presetValues).concat(Object.keys(root.customPresets))

    Flow {
        Layout.fillWidth: true
        spacing: 8

        Repeater {
            model: root.presetNames
            Chip {
                required property string modelData
                readonly property bool builtIn: root.presetValues[modelData] !== undefined
                text: modelData.toLowerCase()
                interactive: true
                on: root.presetName === modelData
                onClicked: builtIn ? root.applyPreset(modelData) : root.applyCustomPreset(modelData)
            }
        }
        // what the curve is called once you have moved a band
        Chip {
            visible: root.presetName === "Custom"
            text: "custom"
            on: true
        }
    }

    RowLayout {
        id: bars
        Layout.fillWidth: true
        Layout.topMargin: 16
        Layout.preferredHeight: 220
        spacing: 10
        uniformCellSizes: true

        Repeater {
            model: root.bandLabels.length

            Item {
                id: band
                required property int index
                readonly property real gain: root.bands[index] ?? 0
                readonly property bool active: mouse.containsMouse || mouse.pressed
                Layout.fillWidth: true
                Layout.fillHeight: true
                activeFocusOnTab: true

                function setGain(g) {
                    const v = Math.max(-root.bandRange, Math.min(root.bandRange, Math.round(g)));
                    if (v !== band.gain) root.setBand(band.index, v);
                }

                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Up) band.setGain(band.gain + 1);
                    else if (event.key === Qt.Key_Down) band.setGain(band.gain - 1);
                    else if (event.key === Qt.Key_Home) band.setGain(root.bandRange);
                    else if (event.key === Qt.Key_End) band.setGain(-root.bandRange);
                    else return;
                    event.accepted = true;
                }

                Rectangle {
                    anchors.fill: parent
                    color: HTokens.inkTrack
                    border.width: HTokens.borderHairline
                    border.color: HTokens.text
                }
                Rectangle {
                    x: HTokens.borderHairline
                    width: parent.width - 2 * HTokens.borderHairline
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: HTokens.borderHairline
                    height: Math.round((parent.height - 2 * HTokens.borderHairline) * (band.gain + root.bandRange) / (2 * root.bandRange))
                    color: HTokens.text
                }
                // the gain, above the bar while you point at it
                HText {
                    visible: band.active
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: -height - 2
                    text: (band.gain > 0 ? "+" : "") + Math.round(band.gain)
                    role: "caption"
                    ink: "c3"
                }
                Rectangle {
                    visible: band.activeFocus
                    anchors.fill: parent
                    anchors.margins: -4
                    color: "transparent"
                    border.width: 2
                    border.color: HTokens.foam
                }
                MouseArea {
                    id: mouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    function apply(y) {
                        band.setGain((1 - y / height) * 2 * root.bandRange - root.bandRange);
                    }
                    onPressed: m => { band.forceActiveFocus(); apply(m.y); }
                    onPositionChanged: m => { if (pressed) apply(m.y); }
                    onWheel: w => band.setGain(band.gain + (w.angleDelta.y > 0 ? 1 : -1))
                }
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: 6
        spacing: 10
        uniformCellSizes: true

        Repeater {
            model: root.bandLabels
            HText {
                required property string modelData
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                text: modelData
                role: "caption"
                ink: "inkMuted"
            }
        }
    }

    Slider {
        id: preampSlider
        Layout.fillWidth: true
        Layout.topMargin: 14
        label: "preamp"
        from: -root.preampRange
        to: root.preampRange
        stepSize: 1
        valueText: (value > 0 ? "+" : "") + Math.round(value)
        onMoved: value => root.setPreamp(value)
        // setValue() replaces the value, so a plain binding would stop following
        Binding {
            target: preampSlider
            property: "value"
            value: root.preamp
            when: !preampSlider.pressed
        }
    }

    HRule {
        Layout.fillWidth: true
        Layout.topMargin: 8
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 8

        Chip {
            text: "auto"
            interactive: true
            on: root.autoEnabled
            onClicked: root.toggleAuto()
        }
        Chip {
            text: "preset: " + root.activePreset
            interactive: root.availablePresets.length > 1
            onClicked: root.cycleActivePreset()
        }
        Item { Layout.fillWidth: true }
        InkButton {
            visible: root.editingCustomPresetName.length > 0
            text: "delete"
            ink: "c2"
            flat: true
            onClicked: root.deleteCustomPreset(root.editingCustomPresetName)
        }
        InkButton {
            visible: root.autoEnabled
            text: "last.fm key"
            flat: true
            onClicked: root.promptMode = root.promptMode === "key" ? "" : "key"
        }
        InkButton {
            text: "save as"
            flat: true
            onClicked: root.promptMode = root.promptMode === "save" ? "" : "save"
        }
        InkButton {
            visible: root.pending
            text: "save"
            ink: "c3"
            onClicked: root.saveChanges()
        }
    }

    PromptLine {
        id: prompt
        visible: root.promptMode.length > 0
        Layout.fillWidth: true
        Layout.topMargin: 6
        path: root.promptMode === "key" ? "~/media/eq/lastfm-key" : "~/media/eq/presets"
        input: true
        onAccepted: {
            if (root.promptMode === "key") root.saveLastfmKey(text);
            else root.saveCustomPreset(text);
        }
        field.Keys.onEscapePressed: root.promptMode = ""
    }

    HText {
        Layout.fillWidth: true
        Layout.topMargin: 6
        text: "easyeffects · preset: " + root.activePreset + " · auto " + (root.autoEnabled ? "on" : "off")
              + (root.pending ? " · unsaved" : "")
        role: "caption"
        ink: root.pending ? "c3" : "c1"
        elide: Text.ElideRight
    }
    HText {
        visible: root.needsManualSave
        Layout.fillWidth: true
        text: "easyeffects has not saved \"" + root.activePreset + "\" yet; save it there first, or pick another preset."
        role: "caption"
        ink: "c2"
        wrapMode: Text.WordWrap
    }
}
