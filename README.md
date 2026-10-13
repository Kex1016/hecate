# hecate

A stacking Wayland compositor written in Rust on Smithay, in the spirit of
[labwc](https://github.com/labwc/labwc) without the Openbox baggage, plus a
bundled desktop shell ([end4-pC](https://github.com/pctrade/end4-pC) on
[illogical-impulse](https://github.com/end-4/dots-hyprland), patched for
hecate) and a Nix flake that installs both.

Focus: QuickShell, gaming (native, Wine, Proton), and NixOS flakes with
config written in pure Nix. Window decorations are just a border with
configurable width and per-corner radii.

## What's in it

- **Stacking window management**: snapping, edge resistance, size presets,
  placement policies, window rules, margins and regions, workspaces per output
  or global, fixed or automatic.
- **Borders and effects**: per-corner rounded borders with content clipping,
  blur behind panels (layer rules, `xray`, `ignore_alpha`) and behind windows
  that ask for it (ext-background-effect), animations for windows, layers,
  workspace switches and border colours.
- **Shell support**: layer-shell, session lock, foreign-toplevel (wlr and ext),
  ext-workspace, screencopy and image-copy-capture, output management and
  power, gamma control, Hyprland's focus grab. Any QuickShell config works.
- **Gaming**: xwayland-satellite on demand, VRR with fullscreen, tearing
  control, drm-syncobj, fifo and commit-timing, DRM leasing for VR headsets,
  `hecate msg gaming` and `hecate msg frames`.
- **Input**: libinput settings per device, touch screens, touchpad gestures
  (three-finger workspace swipes), drawing tablets, input methods.
- **Tooling**: `hecate msg` IPC with JSON output, window identification
  (`pick`, `identify`, `rules`, `tree`), runtime overrides
  (`hecate msg set animations.enable false`), `hecate validate`, a JSON Schema
  for the config.

## Quick start (NixOS)

```nix
# flake.nix
{
  inputs.hecate.url = "github:Kex1016/wl-hex";

  outputs = { nixpkgs, home-manager, hecate, ... }: {
    nixosConfigurations.box = nixpkgs.lib.nixosSystem {
      modules = [
        hecate.nixosModules.default
        { programs.hecate = { enable = true; shell.enable = true; gaming.enable = true; }; }

        home-manager.nixosModules.home-manager
        {
          home-manager.users.you = {
            imports = [ hecate.homeManagerModules.default ];
            wayland.windowManager.hecate = {
              enable = true;
              shell.enable = true;
              settings = {
                border = { width = 2; radius = 12; };
                binds."Mod+T".spawn = [ "kitty" ];
              };
            };
          };
        }
      ];
    };
  };
}
```

Pick "hecate" in your display manager. Without Nix: `cargo build --release`
and put `resources/hecate.desktop` in `/usr/share/wayland-sessions`.

## Docs

- [config.md](docs/config.md): every config option
- [keybinds.md](docs/keybinds.md): the default and shell keybinds
- [nix.md](docs/nix.md): the flake, NixOS and Home Manager modules
- [shell.md](docs/shell.md): the bundled shell and what's patched in it
- [cli.md](docs/cli.md) and [ipc.md](docs/ipc.md): `hecate msg` and the IPC protocol
- [animations.md](docs/animations.md), [protocols.md](docs/protocols.md)
- [PLAN.md](docs/PLAN.md) and [decisions.md](docs/decisions.md): how it came to be

## License

GPL-3.0-or-later. Code adapted from [niri](https://github.com/YaLTeR/niri)
(same license) is marked at the top of each file. The bundled shell under
`shell/` is GPL-3.0 and keeps its upstream license and credits.
