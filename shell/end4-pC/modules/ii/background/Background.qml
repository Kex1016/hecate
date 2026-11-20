pragma ComponentBehavior: Bound

import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.widgetCanvas
import QtQuick
import Quickshell
import Quickshell.Wayland

// The desktop's own layer on every screen: the desktop widgets and the right-click menu.
//
// It has no background of its own. hecate draws the blobs under every layer
// (`background.blobs`), behind the lock screen and in hecate-greetd too, so the same blobs
// keep moving through login and while the shell restarts. With `background.mode`
// "wallpaper" each screen shows its picture here instead (with `background.dither`, the copy
// Wallpapers dithered to two haiiro tones), and once every screen has one the compositor is
// told to stop the blobs nobody sees (`hecate msg blobs off`).
Variants {
    id: root
    model: Quickshell.screens

    readonly property bool ready: Config.ready
    // The login screen always has the blobs, whatever its user picked for the desktop.
    readonly property bool wallpaperMode: ready && !GlobalStates.greeterMode
        && Config.options.background.mode === "wallpaper"
    // Every screen shows its picture.
    readonly property bool covered: wallpaperMode && instances.length > 0
        && Array.from(instances).every(screen => screen.covered)
    // Reduced motion: the blobs are one frame that doesn't move.
    readonly property bool still: ready && Config.options.motion.reduced

    function tellCompositor(args) {
        // The login screen's compositor is hecate-greetd's to set up.
        if (!ready || WM.compositor !== "hecate" || GlobalStates.greeterMode)
            return;
        Quickshell.execDetached(["hecate", "msg"].concat(args));
    }
    // Overrides in the running compositor: its config file is not touched, and they are
    // said again whenever the shell starts.
    function syncBlobs() {
        tellCompositor(["blobs", covered ? "off" : "on"]);
    }
    function syncStill() {
        tellCompositor(still ? ["set", "background.blobs.still", "true"] : ["unset", "background.blobs.still"]);
    }

    onReadyChanged: {
        syncBlobs();
        syncStill();
    }
    onCoveredChanged: syncBlobs()
    onStillChanged: syncStill()
    Component.onCompleted: {
        syncBlobs();
        syncStill();
    }

    PanelWindow {
        id: bgRoot

        required property var modelData
        // This screen's picture in wallpaper mode: the one being tried out in the picker,
        // else its own (`perMonitor`), else everyone's.
        readonly property string wallpaperPath: {
            if (!root.wallpaperMode)
                return "";
            const own = Config.options.background.perMonitor ? Wallpapers.monitorWallpaper(bgRoot.modelData.name) : "";
            return Wallpapers.previewPath || own || Wallpapers.confirmedPath || Config.options.background.wallpaperPath;
        }
        // The screen in device pixels: the picture is decoded, and dithered, at this size.
        readonly property size pixelSize: Qt.size(Math.ceil(bgRoot.modelData.width * bgRoot.modelData.devicePixelRatio),
            Math.ceil(bgRoot.modelData.height * bgRoot.modelData.devicePixelRatio))
        readonly property bool dithered: root.wallpaperMode && Config.options.background.dither
        // What the Image loads. With dithering on it is the copy Wallpapers presses into two
        // tones for this screen's size; until that copy is ready this is "" and the Image
        // keeps what it shows. If the copy can't be made (no ImageMagick), the picture itself.
        readonly property string displayPath: {
            const path = bgRoot.wallpaperPath;
            if (!path || !bgRoot.dithered)
                return path;
            const copy = Wallpapers.ditheredScreenPath(path, bgRoot.pixelSize.width, bgRoot.pixelSize.height);
            if (copy.length > 0)
                return copy;
            return Wallpapers.ditherFailed(path, bgRoot.pixelSize.width, bgRoot.pixelSize.height) ? path : "";
        }
        // The last path worth loading: a copy still being made doesn't blank the screen.
        property string shownPath: ""
        onDisplayPathChanged: {
            if (bgRoot.displayPath.length > 0 || !bgRoot.wallpaperPath)
                bgRoot.shownPath = bgRoot.displayPath;
        }
        Component.onCompleted: bgRoot.shownPath = bgRoot.displayPath
        // The picture is up: nothing the compositor draws below shows on this screen.
        readonly property bool covered: wallpaper.shown && !GlobalStates.startupLockPending

        screen: modelData
        exclusionMode: ExclusionMode.Ignore
        // In hecate-greetd there are no windows to cover, and its password surface must stay on top.
        WlrLayershell.layer: GlobalStates.greeterMode ? WlrLayer.Background : WlrLayer.Bottom
        WlrLayershell.namespace: "quickshell:background"
        WlrLayershell.keyboardFocus: GlobalStates.desktopWidgetKeyboardFocus
            ? WlrKeyboardFocus.OnDemand
            : WlrKeyboardFocus.None
        anchors {
            top: true
            bottom: true
            left: true
            right: true
        }
        // Clear: what shows through is the compositor's background.
        color: "transparent"

        Item {
            anchors.fill: parent
            // Nothing of the desktop shows while a lock screen may still be coming up.
            visible: !GlobalStates.startupLockPending

            Image {
                id: wallpaper

                // Up once it has loaded, and it stays up while the next one loads: a change
                // of picture is a cut, with no blobs in between.
                property bool shown: false

                anchors.fill: parent
                visible: shown
                source: bgRoot.shownPath
                fillMode: Image.PreserveAspectCrop
                // Decoded at the screen's resolution: a 5K picture and its mipmaps stall the
                // render thread on their way to the GPU. A dithered copy is already exactly that
                // size, and is drawn pixel for pixel: no filtering, no mipmaps.
                sourceSize: bgRoot.pixelSize
                asynchronous: true
                retainWhileLoading: true
                cache: true
                smooth: !bgRoot.dithered
                mipmap: !bgRoot.dithered
                onStatusChanged: {
                    if (status === Image.Ready)
                        shown = true;
                    else if (status !== Image.Loading)
                        shown = false;
                }
            }

            WidgetCanvas {
                anchors.fill: parent

                WidgetsLoader {
                    screen: bgRoot.screen
                    wallpaperItem: wallpaper
                    wallpaperSafetyTriggered: false
                }
            }
        }
    }
}
