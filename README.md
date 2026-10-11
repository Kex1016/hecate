# hecate

A stacking Wayland compositor written in Rust on Smithay, in the spirit of
[labwc](https://github.com/labwc/labwc) without the Openbox baggage, plus a
bundled desktop shell ([end4-pC](https://github.com/pctrade/end4-pC) on
[illogical-impulse](https://github.com/end-4/dots-hyprland), patched for
hecate) and a Nix flake that installs both.

Focus: QuickShell, gaming (native, Wine, Proton), and NixOS flakes with
config written in pure Nix. Window decorations are just a border with
configurable width and per-corner radii.

Status: planning. See [docs/PLAN.md](docs/PLAN.md).

License: GPL-3.0-or-later. The bundled shell under `shell/` is GPL-3.0 and
keeps its upstream license and credits.
