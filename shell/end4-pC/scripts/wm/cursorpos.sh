#!/usr/bin/env bash
# Pointer position as {"x", "y"} (what `hyprctl cursorpos -j` gives).
source "$(dirname "$0")/compositor.sh"
case "$(wm_compositor)" in
    hecate) hecate msg -j cursor-position | jq '{x: (.x | floor), y: (.y | floor)}' ;;
    *) hyprctl cursorpos -j ;;
esac
