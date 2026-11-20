pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.services.apple
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

// cat wallpapers.toml · where wallpapers come from: the folder, or an iCloud Photos album
// (services/apple/Photos.qml, fetched with icloudpd). Wallpaper mode only: with the blobs it
// says so and offers nothing. Made for the wallpapers page, above the pictures.
H.HWindow {
    id: root

    property bool live: true
    property bool choosingAlbum: false

    readonly property bool icloud: Photos.selected

    function rotateText(minutes) {
        if (minutes <= 0) return "never rotates";
        if (minutes === 60) return "rotate hourly";
        if (minutes === 1440) return "rotate daily";
        return minutes % 60 === 0 ? `rotate every ${minutes / 60}h` : `rotate every ${minutes} min`;
    }

    label: "cat wallpapers.toml"
    widget: true
    implicitWidth: 360

    // source [folder|icloud album]
    RowLayout {
        visible: Photos.offered
        spacing: H.HTokens.ch(14)
        H.HText { role: "code"; text: "source" }
        H.Segmented {
            model: ["folder", "icloud album"]
            currentIndex: root.icloud ? 1 : 0
            onActivated: i => Photos.setSource(i === 1 ? "icloudAlbum" : "folder")
        }
    }

    // album Lake trip · 48 photos
    RowLayout {
        Layout.fillWidth: true
        visible: Photos.offered && root.icloud
        spacing: H.HTokens.ch(14)
        H.HText { role: "code"; ink: "c3"; text: "album" }
        H.HText {
            Layout.fillWidth: true
            role: "code"
            elide: Text.ElideRight
            text: (Photos.album || "none picked")
                + (Photos.count > 0 ? ` · ${Photos.count} photos` : Photos.syncing ? " · fetching" : "")
            TapHandler {
                onTapped: {
                    root.choosingAlbum = !root.choosingAlbum;
                    if (root.choosingAlbum && Photos.albums.length === 0) Photos.listAlbums();
                }
            }
        }
    }

    // the albums, as chips, while picking one
    Flow {
        Layout.fillWidth: true
        visible: Photos.offered && root.icloud && root.choosingAlbum
        spacing: 6
        Repeater {
            model: Photos.albums
            delegate: H.Chip {
                required property string modelData
                text: modelData.toLowerCase()
                on: modelData === Photos.album
                interactive: true
                onClicked: {
                    Photos.setAlbum(modelData);
                    root.choosingAlbum = false;
                }
            }
        }
        H.HText {
            visible: Photos.albums.length === 0
            role: "caption"
            ink: "c1"
            text: Photos.listing ? "asking icloud…" : "no albums yet · sign in first"
        }
    }

    // rotate hourly · dithered to haiiro
    RowLayout {
        visible: Photos.offered && root.icloud
        spacing: H.HTokens.ch(14)
        H.HText { role: "code"; ink: "c3"; text: root.rotateText(Photos.rotateMinutes).split(" ")[0] }
        H.HText {
            role: "code"
            text: root.rotateText(Photos.rotateMinutes).split(" ").slice(1).join(" ")
                + (Wallpapers.dither ? " · dithered to haiiro" : " · as it is")
        }
    }

    // signing in, or what went wrong
    RowLayout {
        Layout.fillWidth: true
        visible: Photos.offered && root.icloud && (Photos.status === "signIn" || Photos.status === "error"
            || Photos.status === "missing" || !Photos.appleId)
        spacing: 8
        H.HText {
            Layout.fillWidth: true
            role: "code"
            ink: Photos.status === "error" ? "c2" : "c4"
            wrapMode: Text.Wrap
            text: Photos.status === "missing" ? "icloudpd is not installed"
                : !Photos.appleId ? "add an iCloud calendar account first, or set apple.appleId"
                : Photos.message || "sign in to iCloud"
        }
        H.InkButton {
            visible: Photos.status !== "missing" && Photos.appleId.length > 0
            ink: "c3"
            role: "code"
            text: "sign in"
            onClicked: Photos.signIn()
        }
    }

    H.HText {
        Layout.fillWidth: true
        role: "caption"
        ink: "c1"
        wrapMode: Text.Wrap
        text: Photos.offered ? "only when background = wallpaper · fetched with icloudpd"
            : "background is blobs · turn wallpapers on to pick a source"
    }
}
