pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.modules.common
import QtQuick
import qs.modules.common.widgets.haiiro as H

// The picture grid: five to a row, the current one in a pine double frame,
// the cursor in a foam outline. Folders are left out; there is one folder.
Item {
    id: root

    property int columns: 5
    property bool dither: Wallpapers.dither
    // path of the picture that is set for the monitor being edited
    property string currentPath: ""
    property var files: []
    signal picked(string path)

    function rebuild() {
        const out = [];
        for (let i = 0; i < Wallpapers.wallpaperModel.count; i++) {
            const it = Wallpapers.wallpaperModel.get(i);
            if (!it.fileIsDir && it.filePath) out.push({ name: it.fileName, path: it.filePath });
        }
        root.files = out;
        if (grid.currentIndex >= out.length) grid.currentIndex = Math.max(0, out.length - 1);
    }
    Component.onCompleted: rebuild()
    Connections {
        target: Wallpapers
        function onResultsUpdated() { root.rebuild(); }
    }

    function moveSelection(delta) {
        grid.currentIndex = Math.max(0, Math.min(root.files.length - 1, grid.currentIndex + delta));
    }
    function activateCurrent() {
        const f = root.files[grid.currentIndex];
        if (f) root.picked(f.path);
    }
    function randomPick() {
        if (root.files.length === 0) return;
        root.picked(root.files[Math.floor(Math.random() * root.files.length)].path);
    }

    readonly property int gap: 20

    GridView {
        id: grid
        anchors.fill: parent
        clip: true
        cellWidth: Math.floor(width / root.columns)
        cellHeight: 150 + 6 + 16 + root.gap
        model: root.files
        currentIndex: 0
        highlightFollowsCurrentItem: false
        boundsBehavior: Flickable.StopAtBounds

        delegate: Item {
            id: cell
            required property var modelData
            required property int index
            readonly property bool current: modelData.path === root.currentPath
            readonly property bool cursor: GridView.isCurrentItem
            width: grid.cellWidth
            height: grid.cellHeight

            Column {
                x: 0
                width: parent.width - root.gap
                spacing: 6

                Item {
                    width: parent.width
                    height: 150

                    Rectangle {
                        anchors.fill: parent
                        color: H.HTokens.base
                    }
                    Image {
                        anchors.fill: parent
                        anchors.margins: cell.current ? 5 : 2
                        source: "file://" + cell.modelData.path
                        sourceSize: Qt.size(480, 300)
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                        smooth: false
                        cache: true
                    }
                    // dither preview: the checker over the picture. The background
                    // layer does the real two-tone pressing.
                    H.Dither {
                        visible: root.dither
                        anchors.fill: parent
                        anchors.margins: cell.current ? 5 : 2
                        ground: "transparent"
                        ink: Qt.rgba(H.HTokens.base.r, H.HTokens.base.g, H.HTokens.base.b, 0.45)
                    }
                    // 2px highlight-high frame; the current one gets pine 5px double
                    Rectangle {
                        visible: !cell.current
                        anchors.fill: parent
                        color: "transparent"
                        border.width: 2
                        border.color: H.HTokens.highlightHigh
                    }
                    Repeater {
                        model: cell.current ? [0, 3] : []
                        Rectangle {
                            required property int modelData
                            x: modelData; y: modelData
                            width: parent.width - 2 * modelData
                            height: parent.height - 2 * modelData
                            color: "transparent"
                            border.width: 2
                            border.color: H.HTokens.pine
                        }
                    }
                    Rectangle {
                        visible: cell.cursor
                        anchors.fill: parent
                        anchors.margins: -4
                        color: "transparent"
                        border.width: 2
                        border.color: H.HTokens.foam
                    }
                }
                H.HText {
                    width: parent.width
                    elide: Text.ElideRight
                    text: cell.modelData.name + (cell.current ? " · current" : "")
                    role: "caption"
                    ink: "subtle"
                }
            }

            MouseArea {
                anchors.fill: parent
                onClicked: { grid.currentIndex = cell.index; root.picked(cell.modelData.path); }
            }
        }
    }
}
