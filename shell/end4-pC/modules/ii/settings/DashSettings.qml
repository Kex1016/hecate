pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

// The dashboard's settings page: the title and a `grep` prompt that searches
// every setting, the `ls` rail of pages, and the open page as
// `cat <page>.toml`. ↑ ↓ move, → or enter goes into a page, ← or esc comes
// back to the rail; typing anywhere searches.
FocusScope {
    id: root

    signal closeRequested()

    readonly property var pages: SettingsPages.pages
    readonly property string currentId: root.pages.some(p => p.id === GlobalStates.dashboardGroup) ? GlobalStates.dashboardGroup : (root.pages[0]?.id ?? "general")
    readonly property var current: root.pages.find(p => p.id === root.currentId) ?? null
    readonly property string query: prompt.text
    readonly property var results: root.query.trim() === "" ? [] : SettingsSearchIndex.search(root.query, 40)
    property int resultIndex: 0
    // where focus is: "rail", "page" or "search"
    property string zone: "rail"

    // The quiet text after a page in the rail: how many settings it has, or
    // what is worth knowing at a glance.
    function meta(id) {
        if (id === "calendars") {
            const kinds = [...new Set(CalendarAccounts.accounts.map(a => a.kind))];
            return kinds.length > 0 ? kinds.join(" · ") : "none";
        }
        if (id === "displays") {
            const n = HecateSettings.outputs.length || Quickshell.screens.length;
            return n === 1 ? "1 screen" : `${n} screens`;
        }
        if (id === "about") return Updates.count > 0 ? `↑${Updates.count}` : "";
        const n = SettingsSearchIndex.count(id);
        return n > 0 ? String(n) : "";
    }

    function select(id) {
        GlobalStates.dashboardGroup = id;
    }
    function moveRail(dir) {
        const i = root.pages.findIndex(p => p.id === root.currentId);
        const next = root.pages[Math.max(0, Math.min(root.pages.length - 1, i + dir))];
        if (next) root.select(next.id);
    }
    function enterPage() {
        if (pageLoader.item?.focusFirst()) root.zone = "page";
    }
    function backToRail() {
        root.zone = "rail";
        rail.forceActiveFocus();
    }

    // Called by the dashboard when typing starts on the home page.
    function startSearch(text) {
        root.zone = "search";
        prompt.text = text;
        prompt.forceInputFocus();
    }
    function endSearch() {
        prompt.text = "";
        root.backToRail();
    }
    function openResult(r) {
        if (!r) return;
        prompt.text = "";
        root.select(r.pageId);
        root.zone = "page";
        Qt.callLater(() => {
            if (!(r.label && pageLoader.item?.reveal(r.label))) pageLoader.item?.focusFirst();
        });
    }

    // A setting the launcher (or a tile) asked for.
    function applyTarget() {
        const t = GlobalStates.settingsTarget;
        if (!t) return;
        GlobalStates.settingsTarget = null;
        root.openResult({ pageId: t.page, label: t.label });
    }
    Connections {
        target: GlobalStates
        function onSettingsTargetChanged() {
            if (GlobalStates.settingsTarget) Qt.callLater(root.applyTarget);
        }
    }
    Component.onCompleted: {
        if (GlobalStates.settingsTarget) Qt.callLater(root.applyTarget);
        else rail.forceActiveFocus();
    }
    onActiveFocusChanged: if (activeFocus && root.zone === "rail") rail.forceActiveFocus()

    function typed(event) {
        if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)) return false;
        return event.text.length === 1 && event.text.charCodeAt(0) > 32 && event.text.charCodeAt(0) !== 127;
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 18

        // title and prompt
        RowLayout {
            Layout.fillWidth: true
            spacing: H.HTokens.space6

            DashTitle {
                Layout.fillWidth: true
                text: "settings"
            }
            DashGrep {
                id: prompt
                Layout.alignment: Qt.AlignBottom
                Layout.bottomMargin: 12
                Layout.preferredWidth: 420
                onKeyPressed: event => {
                    if (event.key === Qt.Key_Escape) root.endSearch();
                    else if (event.key === Qt.Key_Down) root.resultIndex = Math.min(root.results.length - 1, root.resultIndex + 1);
                    else if (event.key === Qt.Key_Up) root.resultIndex = Math.max(0, root.resultIndex - 1);
                    else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) root.openResult(root.results[root.resultIndex]);
                    else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) event.accepted = false;
                    else return;
                    if (event.key !== Qt.Key_Tab && event.key !== Qt.Key_Backtab) event.accepted = true;
                }
                onTextChanged: root.resultIndex = 0
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: H.HTokens.space6

            // the rail
            H.HWindow {
                Layout.preferredWidth: 280
                Layout.fillHeight: true
                label: "ls"

                FocusScope {
                    id: rail
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.topMargin: -4

                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_Up) root.moveRail(-1);
                        else if (event.key === Qt.Key_Down) root.moveRail(1);
                        else if (event.key === Qt.Key_Right || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) root.enterPage();
                        else if (event.key === Qt.Key_Escape) root.closeRequested();
                        else if (event.key === Qt.Key_Slash) root.startSearch("");
                        else if (root.typed(event)) root.startSearch(event.text);
                        else return;
                        event.accepted = true;
                    }

                    Column {
                        width: parent.width

                        Repeater {
                            model: root.pages
                            Column {
                                id: entry
                                required property int index
                                required property var modelData
                                width: parent.width

                                // the hecate pages under their own heading, about at the end
                                Item {
                                    visible: entry.index > 0 && root.pages[entry.index - 1].group !== entry.modelData.group
                                    width: parent.width
                                    height: rule.height + (head.visible ? head.height : 0)
                                    H.HRule {
                                        id: rule
                                        width: parent.width
                                    }
                                    H.HLabel {
                                        id: head
                                        visible: entry.modelData.group === "hecate"
                                        y: rule.height
                                        x: 8
                                        text: "hecate"
                                        height: implicitHeight + 4
                                    }
                                }
                                H.ListRow {
                                    width: parent.width
                                    text: entry.modelData.name
                                    meta: root.meta(entry.modelData.id)
                                    selected: entry.modelData.id === root.currentId
                                    onClicked: {
                                        root.select(entry.modelData.id);
                                        root.backToRail();
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // the page, or the search results
            H.HWindow {
                id: pageWindow
                Layout.fillWidth: true
                Layout.fillHeight: true
                label: root.query !== "" ? `grep -ri "${root.query}"` : `cat ${root.currentId.replace(/ /g, "-")}.toml`
                hints: root.query !== "" ? [{ key: "↑↓", text: "move" }, { key: "enter", text: "open" }, { key: "esc", text: "back" }]
                    : (pageLoader.item?.hints ?? [{ key: "↑↓", text: "move" }, { key: "space", text: "toggle" }, { key: "←→", text: "choose" }, { key: "/", text: "search" }])
                note: root.query !== "" ? `${root.results.length} found` : (pageLoader.item?.note ?? "")

                Loader {
                    id: pageLoader
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    visible: root.query === ""
                    source: root.current?.component ?? ""
                    focus: root.zone === "page"

                    Connections {
                        target: pageLoader.item
                        ignoreUnknownSignals: true
                        function onLeaveRequested() {
                            root.backToRail();
                        }
                    }
                    // ← or esc inside a page goes back to the rail.
                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_Escape || event.key === Qt.Key_Left && !(event.modifiers & Qt.ShiftModifier)) {
                            root.backToRail();
                            event.accepted = true;
                        } else if (event.key === Qt.Key_Slash) {
                            root.startSearch("");
                            event.accepted = true;
                        }
                    }
                }

                ListView {
                    id: resultList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    visible: root.query !== ""
                    clip: true
                    model: root.results
                    currentIndex: root.resultIndex
                    onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)
                    delegate: H.ListRow {
                        id: hit
                        required property int index
                        required property var modelData
                        width: resultList.width
                        text: modelData.kind === "page" ? modelData.pageName : modelData.label
                        match: root.query.trim().split(/\s+/)[0] ?? ""
                        meta: modelData.kind === "page" ? "page" : [modelData.pageName, modelData.section].filter(s => s).join(" › ")
                        selected: index === root.resultIndex
                        onEntered: root.resultIndex = index
                        onClicked: root.openResult(modelData)
                    }
                }
                H.HText {
                    visible: root.query !== "" && root.results.length === 0
                    text: "Nothing by that name. haiiro has no theme to search for, if that is what you wanted."
                    ink: "rose"
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
            }
        }
    }
}
