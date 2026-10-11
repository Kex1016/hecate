pragma Singleton

import qs
import QtQuick
import Quickshell

Singleton {
    id: root

    // Installed from a Nix store path (hecate's flake): read-only, updated with the system.
    readonly property bool managedByNix: String(Quickshell.shellPath("")).includes("/nix/store/")

    function notifyManaged() {
        Quickshell.execDetached(["notify-send", "Updates", "This shell is installed by Nix. Update the flake and rebuild instead."])
        Qt.callLater(() => GlobalStates.settingsOpen = false)
    }

    function runSystemUpdate() {
        if (root.managedByNix) return root.notifyManaged();
        Quickshell.execDetached([
            "kitty", "--hold",
            "fish", "-i", "-l", "-c",
            "yay -Syu --combinedupgrade=false"
        ])
        Qt.callLater(() => GlobalStates.settingsOpen = false)
    }

    function runUpdateDots() {
        if (root.managedByNix) return root.notifyManaged();
        const updateScript = `
            set -e
            DIR="$HOME/.config/quickshell"

            rm -rf "$DIR/end4-pC-tmp"
            git clone https://github.com/pctrade/end4-pC.git "$DIR/end4-pC-tmp"

            rm -rf "$DIR/end4-pC-old"
            [ -d "$DIR/end4-pC" ] && mv "$DIR/end4-pC" "$DIR/end4-pC-old"
            mv "$DIR/end4-pC-tmp" "$DIR/end4-pC"

            killall qs 2>/dev/null || true
            sleep 0.5
            setsid qs -c end4-pC >/tmp/qs.log 2>&1 < /dev/null &
            disown

            rm -rf "$DIR/end4-pC-old"
        `

        Quickshell.execDetached(["kitty", "--hold", "bash", "-c", updateScript])
        Qt.callLater(() => GlobalStates.settingsOpen = false)
    }
}
