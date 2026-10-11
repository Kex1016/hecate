# Name

**Decided: hecate** (round 2). Goddess of witchcraft, crossroads and
thresholds, and it keeps the "hex" sound of the original `wl-hex`.

Applied as:

| knob                   | value                                   |
| ---------------------- | --------------------------------------- |
| binary                 | `hecate` (compositor and `hecate msg`)  |
| crate(s)               | `hecate`, `hecate-ipc`                  |
| config dir             | `~/.config/hecate/`                     |
| env vars               | `HECATE_SOCKET`, `HECATE_LOG`, `HECATE_CONFIG` |
| `XDG_CURRENT_DESKTOP`  | `hecate`                                |
| session file           | `share/wayland-sessions/hecate.desktop` |
| Nix                    | `programs.hecate`, `wayland.windowManager.hecate`, `pkgs.hecate` |
| repo                   | still `kex1016/wl-hex`; rename to `hecate` on GitHub when convenient (GitHub redirects the old URL) |

Known clash: a small hex editor called "hecate" (Go, binary `hecate`).
It isn't in nixpkgs, so there's no conflict on NixOS. We'll check crates.io
before publishing anything.

Runners-up from the round 1 list, for sub-projects if we ever need them:
cauldron, besom, glamour, coven, sigil, scry, grimoire, familiar.
