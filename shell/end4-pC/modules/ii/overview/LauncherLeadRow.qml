pragma ComponentBehavior: Bound
import QtQuick
import qs.services
import qs.modules.common.widgets.haiiro as H

// A launcher row with something before its text: the clipboard number or
// pinned star, a placeholder for a copied image, or the gold verb of an
// action (`search the web` for "fir"). Plain rows stay LauncherRow, so they
// keep no empty leading slot.
LauncherRow {
    id: root

    // 1-9 in the clipboard, -1 for none
    property int number: -1

    leading: [
        H.HText {
            visible: LauncherSearch.mode === "clipboard"
            width: H.HTokens.ch()
            text: root.entry.pinned ? "★" : (root.number > 0 ? String(root.number) : "")
            ink: root.entry.pinned ? "gold" : "muted"
            anchors.verticalCenter: parent ? parent.verticalCenter : undefined
        },
        H.Dither {
            visible: root.entry.image === true
            width: root.entry.image ? 160 : 0
            height: root.entry.image ? 90 : 0
            big: true
        },
        H.HText {
            visible: root.entry.image === true
            text: "image"
            ink: "iris"
            anchors.verticalCenter: parent ? parent.verticalCenter : undefined
        },
        H.HText {
            visible: root.entry.image === true && (root.entry.imageInfo ?? "").length > 0
            text: root.entry.imageInfo ?? ""
            ink: "muted"
            anchors.verticalCenter: parent ? parent.verticalCenter : undefined
        },
        H.HText {
            visible: (root.entry.verb ?? "").length > 0
            text: root.entry.verb ?? ""
            ink: "gold"
            anchors.verticalCenter: parent ? parent.verticalCenter : undefined
        }
    ]

}
