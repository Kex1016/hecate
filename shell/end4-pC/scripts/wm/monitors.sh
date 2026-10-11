#!/usr/bin/env bash
# Monitors as Hyprland-shaped JSON on any compositor (what `hyprctl monitors -j` gives):
# [{"name", "width", "height", "x", "y", "scale", "focused"}]
source "$(dirname "$0")/compositor.sh"
case "$(wm_compositor)" in
    hecate)
        hecate msg -j outputs | jq '[to_entries[] | .value | select(.logical != null)
            | {name, width: .modes[.current_mode].width, height: .modes[.current_mode].height,
               x: .logical.x, y: .logical.y, scale: .logical.scale, focused: .is_focused}]'
        ;;
    niri)
        focused="$(niri msg -j focused-output | jq -r '.name')"
        niri msg -j outputs | jq --arg f "$focused" '[to_entries[] | .value | select(.logical != null)
            | {name, width: .modes[.current_mode].width, height: .modes[.current_mode].height,
               x: .logical.x, y: .logical.y, scale: .logical.scale, focused: (.name == $f)}]'
        ;;
    *)
        hyprctl monitors -j
        ;;
esac
