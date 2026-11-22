import QtQuick
import qs.modules.common.widgets.haiiro

// The one open and close effect every panel, popup, toast and widget uses.
// Things pixelate: open shows the content at its full size in 16px cells,
// then 8, 4, 2 and sharp, one step every 125ms (500ms in all, no easing);
// close plays the same frames backwards and then cuts to nothing. Each cell
// is the average of the pixels under it. Nothing slides or fades, and with
// HTokens.cut (Game Mode, reduced motion) both are a single cut.
//
//   PixelTransition {
//       shown: panel.open
//       HWindow { label: "hecate: ~/launch"; ... }
//   }
//
// Group items that pixelate in together (list rows, widget tiles) set
// `index`; each waits index x 60ms. Keep the surface mapped while `present`
// is true, so the close frames have somewhere to draw.
Item {
    id: root

    property bool shown: true
    property int index: 0
    // pixelate on creation when shown starts true (a panel built by a Loader)
    property bool animateOnLoad: true
    // drawn area around the content, for the notch label above a window
    property int margin: 9
    default property alias content: holder.data

    readonly property int steps: Math.max(1, HTokens.pixelateSteps)
    readonly property int stepMs: Math.round(HTokens.pixelateDuration / steps)
    // 0: nothing on screen, 1: sharp, >1: the size of a cell in pixels
    // set once on creation (a binding to `shown` would race the close)
    property real cell: 0
    readonly property bool animating: delay.running || stepper.running
    readonly property bool present: shown || cell !== 0
    signal opened()
    signal closed()

    property var sequence: []
    property int position: 0

    implicitWidth: holder.childrenRect.width
    implicitHeight: holder.childrenRect.height

    function frames(open) {
        const out = [];
        for (let i = 0; i < root.steps; i++)
            out.push(HTokens.pixelateCellStart / Math.pow(2, i));
        // open: 16, 8, 4, 2, sharp; close: 2, 4, 8, 16, gone
        return open ? out.concat([1]) : out.reverse().concat([0]);
    }

    function play() {
        delay.stop();
        stepper.stop();
        // nothing on screen yet (created hidden, or an open still waiting):
        // a close has nothing to play
        if (!root.shown && root.cell === 0)
            return;
        if (HTokens.cut) {
            root.cell = root.shown ? 1 : 0;
            root.shown ? root.opened() : root.closed();
            return;
        }
        root.sequence = root.frames(root.shown);
        root.position = 0;
        delay.interval = Math.max(0, root.index) * HTokens.pixelateStagger;
        if (delay.interval > 0) {
            // an open waits off screen, a close waits sharp
            root.cell = root.shown ? 0 : (root.cell === 0 ? 0 : 1);
            delay.start();
        } else {
            root.begin();
        }
    }

    function begin() {
        root.cell = root.sequence[0];
        root.position = 1;
        stepper.start();
    }

    onShownChanged: play()
    Component.onCompleted: {
        if (!shown) return;
        if (animateOnLoad) play();
        else cell = 1;
    }

    Timer {
        id: delay
        onTriggered: root.begin()
    }

    Timer {
        id: stepper
        interval: root.stepMs
        repeat: true
        onTriggered: {
            root.cell = root.sequence[root.position];
            root.position += 1;
            if (root.position >= root.sequence.length) {
                stop();
                root.cell === 0 ? root.closed() : root.opened();
            }
        }
    }

    Item {
        id: holder
        anchors.fill: parent
        visible: root.cell !== 0
    }

    // While pixelated: capture the content (mipmapped, so a small read is an
    // average), read it back at one texel per cell, and draw that texture
    // scaled up with nearest-neighbour.
    readonly property bool pixelated: root.cell > 1
    readonly property int areaW: Math.ceil(root.width + 2 * root.margin)
    readonly property int areaH: Math.ceil(root.height + 2 * root.margin)

    ShaderEffectSource {
        id: capture
        visible: root.pixelated
        sourceItem: root.pixelated ? holder : null
        hideSource: root.pixelated
        sourceRect: Qt.rect(-root.margin, -root.margin, root.areaW, root.areaH)
        x: -root.margin
        y: -root.margin
        width: root.areaW
        height: root.areaH
        mipmap: true
        smooth: true
        live: true
    }

    ShaderEffectSource {
        id: cells
        visible: root.pixelated
        sourceItem: root.pixelated ? capture : null
        hideSource: root.pixelated
        readonly property int tw: Math.max(1, Math.ceil(root.areaW / Math.max(1, root.cell)))
        readonly property int th: Math.max(1, Math.ceil(root.areaH / Math.max(1, root.cell)))
        textureSize: Qt.size(tw, th)
        x: -root.margin
        y: -root.margin
        width: tw * root.cell
        height: th * root.cell
        smooth: false
        mipmap: false
        live: true
    }
}
