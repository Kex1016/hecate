pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

// What the dashboard shows: the veil, the current page and the nav. Home and
// settings are pages here; media and (in wallpaper mode) wallpapers open
// their own panels.
FocusScope {
    id: root

    property bool open: true
    signal closeRequested()

    // 0 home, 1 settings (GlobalStates keeps it across openings)
    readonly property int page: GlobalStates.dashboardPage === 1 ? 1 : 0
    readonly property bool wallpaperMode: Config.options.background.mode === "wallpaper"

    focus: true

    function goTo(index) {
        GlobalStates.dashboardPage = index;
        Qt.callLater(() => (index === 1 ? settingsLoader : homeLoader).item?.forceActiveFocus());
    }

    function openPanel(name) {
        root.closeRequested();
        if (name === "media") GlobalStates.mediaControlsOpen = true;
        else if (name === "wallpapers") GlobalStates.wallpaperSelectorOpen = true;
    }

    // A printable key without modifiers: the start of a settings search.
    function typed(event) {
        if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)) return false;
        return event.text.length === 1 && event.text.charCodeAt(0) > 32 && event.text.charCodeAt(0) !== 127;
    }

    onOpenChanged: if (open) Qt.callLater(() => root.forceActiveFocus())

    Keys.onPressed: event => {
        if (event.key === Qt.Key_Escape) {
            root.closeRequested();
        } else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) {
            root.goTo(root.page === 0 ? 1 : 0);
        } else if (root.page === 0 && (root.typed(event) || event.key === Qt.Key_Slash)) {
            root.goTo(1);
            Qt.callLater(() => settingsLoader.item?.startSearch(event.key === Qt.Key_Slash ? "" : event.text));
        } else {
            return;
        }
        event.accepted = true;
    }

    H.Veil {
        anchors.fill: parent
        heavy: true
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.topMargin: H.HTokens.space8
        anchors.leftMargin: H.HTokens.space12
        anchors.rightMargin: H.HTokens.space12
        anchors.bottomMargin: H.HTokens.space6
        spacing: 18

        Loader {
            id: homeLoader
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.page === 0
            active: root.page === 0
            focus: root.page === 0
            sourceComponent: DashHome {
                focus: true
                onOpenSettings: id => {
                    GlobalStates.dashboardGroup = id;
                    root.goTo(1);
                }
                onOpenPanel: name => root.openPanel(name)
            }
        }

        Loader {
            id: settingsLoader
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.page === 1
            active: root.page === 1
            focus: root.page === 1
            sourceComponent: DashSettings {
                focus: true
                onCloseRequested: root.closeRequested()
            }
        }

        DashNav {
            Layout.fillWidth: true
            current: root.page === 0 ? "home" : "settings"
            items: root.wallpaperMode ? ["home", "settings", "wallpapers", "media"] : ["home", "settings", "media"]
            hint: root.page === 0 ? "home" : "settings"
            onPicked: name => {
                if (name === "home") root.goTo(0);
                else if (name === "settings") root.goTo(1);
                else root.openPanel(name);
            }
        }
    }
}
