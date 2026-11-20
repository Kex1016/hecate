import QtQuick
import QtQuick.Layouts
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

// A settings row bound to its value. `key` is a path under Config.options
// ("background.mode"); `hecateKey` is a compositor key that HecateSettings
// saves to ~/.config/hecate/shell.toml ("focus.follow_mouse"); without
// either, `value` is set by the owner and `edited` reports changes.
// SetToggle, SetChoice, SetNumber, SetText and SetAction put the control in.
//
// The search index reads the page files: keep `name`, `description`, `key`,
// `hecateKey`, `values`, `labels`, `from`, `to`, `stepSize`, `divisor` and
// `suffix` as plain literals, one per line.
Item {
    id: root

    property alias name: row.name
    property alias description: row.description
    default property alias control: row.control
    property string key: ""
    property string hecateKey: ""
    property var fallback: undefined
    property var value: undefined
    readonly property var current: key !== "" ? root.read(key)
        : hecateKey !== "" ? HecateSettings.value(hecateKey, fallback)
        : value
    // lit up for a moment when search or a link lands on this row
    property bool flash: false
    readonly property bool isSetRow: true
    // false: a subtype draws its own layout (SetRichRow)
    property bool plainLayout: true
    // false keeps the row out of the settings search (form fields, for one)
    property bool searchable: true
    signal edited(var value)

    Layout.fillWidth: true
    implicitWidth: row.implicitWidth
    implicitHeight: plainLayout ? row.implicitHeight : 0

    function read(path) {
        const v = path.split(".").reduce((o, k) => o?.[k], Config.options);
        return v === undefined ? root.fallback : v;
    }

    function write(v) {
        if (root.key !== "") Config.setNestedValue(root.key, v);
        else if (root.hecateKey !== "") HecateSettings.set(root.hecateKey, v);
        root.edited(v);
    }

    // The first focusable item in the control slot.
    function focusTarget() {
        const walk = item => {
            for (const c of item.children) {
                if (c.activeFocusOnTab && c.visible && c.enabled) return c;
                const deeper = walk(c);
                if (deeper) return deeper;
            }
            return null;
        };
        return walk(root);
    }
    function takeFocus() {
        const t = root.focusTarget();
        if (t) t.forceActiveFocus();
        return t !== null;
    }
    readonly property bool focused: {
        let item = root.Window.activeFocusItem;
        while (item) {
            if (item === root) return true;
            item = item.parent;
        }
        return false;
    }

    // ↑ ↓ from any control: the page moves to the row above or below.
    function navigate(dir) {
        let item = root.parent;
        while (item && typeof item.navigateFrom !== "function") item = item.parent;
        if (item) item.navigateFrom(root, dir);
    }
    Keys.onUpPressed: event => {
        root.navigate(-1);
        event.accepted = true;
    }
    Keys.onDownPressed: event => {
        root.navigate(1);
        event.accepted = true;
    }

    function blink() {
        root.flash = true;
        flashTimer.restart();
    }
    Timer {
        id: flashTimer
        interval: 1600
        onTriggered: root.flash = false
    }

    // the selected-row band of the kit's lists, with the rose » in the gutter
    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: -H.HTokens.space3
        anchors.rightMargin: -H.HTokens.space3
        visible: root.flash || root.focused
        color: H.HTokens.overlay
    }
    H.HText {
        x: -H.HTokens.space3 + 2
        y: 8 + H.HTokens.borderHairline
        visible: root.flash || root.focused
        text: "»"
        ink: "rose"
    }

    H.SettingRow {
        id: row
        visible: root.plainLayout
        width: parent.width
        enabled: root.enabled
    }
}
