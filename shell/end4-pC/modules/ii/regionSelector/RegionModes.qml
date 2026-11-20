pragma Singleton
import Quickshell

// The modes of the region bar, in the order Tab walks them. `action` is the SnipAction a mode
// starts with; shot also has Edit (annotate) and record also has RecordWithSound.
Singleton {
    id: root

    readonly property var modes: [
        { id: "shot", action: RegionSelection.SnipAction.Copy },
        { id: "ocr", action: RegionSelection.SnipAction.CharRecognition },
        { id: "search", action: RegionSelection.SnipAction.Search },
        { id: "ask", action: RegionSelection.SnipAction.Ask },
        { id: "record", action: RegionSelection.SnipAction.Record },
        { id: "translate", action: RegionSelection.SnipAction.Translate },
        { id: "pick", action: RegionSelection.SnipAction.Pick }
    ]

    function idOf(action) {
        switch (action) {
        case RegionSelection.SnipAction.Copy:
        case RegionSelection.SnipAction.Edit:
            return "shot";
        case RegionSelection.SnipAction.CharRecognition:
            return "ocr";
        case RegionSelection.SnipAction.Search:
            return "search";
        case RegionSelection.SnipAction.Ask:
            return "ask";
        case RegionSelection.SnipAction.Record:
        case RegionSelection.SnipAction.RecordWithSound:
            return "record";
        case RegionSelection.SnipAction.Translate:
            return "translate";
        case RegionSelection.SnipAction.Pick:
            return "pick";
        }
        return "shot";
    }

    function indexOf(action) {
        const id = root.idOf(action);
        return root.modes.findIndex(m => m.id === id);
    }

    // The line at the left of the footer: what a drag or a click does in this mode.
    function description(action, selectionMode) {
        if (action === RegionSelection.SnipAction.Pick)
            return "click a pixel to copy its colour";
        if (selectionMode === RegionSelection.SelectionMode.ScreenTarget)
            return "click to use this screen";
        if (selectionMode === RegionSelection.SelectionMode.Circle)
            return "draw around it";
        return "drag to select · click a window to take it";
    }

    // The right of the footer: where the result goes.
    function note(action) {
        switch (action) {
        case RegionSelection.SnipAction.Copy:
        case RegionSelection.SnipAction.Edit:
            return "";
        case RegionSelection.SnipAction.CharRecognition:
            return "text to the clipboard";
        case RegionSelection.SnipAction.Search:
            return "opens in the browser";
        case RegionSelection.SnipAction.Ask:
            return "claude: ~/ask";
        case RegionSelection.SnipAction.Record:
        case RegionSelection.SnipAction.RecordWithSound:
            return "stop it from the rec window";
        case RegionSelection.SnipAction.Translate:
            return "text to the clipboard";
        case RegionSelection.SnipAction.Pick:
            return "hex to the clipboard";
        }
        return "";
    }
}
