#!/usr/bin/env bash
# Print the running compositor: hecate, niri or hyprland. Sourced or run.
wm_compositor() {
    local desktop="${XDG_CURRENT_DESKTOP,,} ${XDG_SESSION_DESKTOP,,}"
    if [[ -n "$HECATE_SOCKET" || "$desktop" == *hecate* ]]; then echo hecate
    elif [[ "$desktop" == *niri* ]]; then echo niri
    else echo hyprland
    fi
}
[[ "${BASH_SOURCE[0]}" == "$0" ]] && wm_compositor
