# Decisions

Log of review decisions on the plan. Newest first.

## Round 2

| topic          | decision                                         | notes |
| -------------- | ------------------------------------------------ | ----- |
| Name           | **hecate**                                       | See [names.md](names.md) for where it applies. Repo rename pending |
| Workspaces     | `scope` = `global` / `per_output`, `kind` = `fixed` / `auto` | Fixed takes `count` (default 9, max 32); auto keeps one empty workspace at the end, max 32 |
| Shell          | **bundle end4-pC + illogical-impulse** in a monorepo, patched for hecate, packaged for NixOS | git subtrees under `shell/`. Both GPL-3.0. See [shell.md](shell.md) |
| Noctalia       | supported through protocols, not bundled         | Native backend after v1 if wanted |
| Layer rules    | added (follows from the shell choice)            | end-4's frosted panels come from Hyprland layer rules (blur, ignore_alpha, xray), so hecate needs the same |

## Round 1

| topic          | decision                                         | notes |
| -------------- | ------------------------------------------------ | ----- |
| Name           | open, candidates in [names.md](names.md)         | superseded in round 2 |
| Config format  | **TOML** primary, JSON accepted, no Lua          | Must be configurable from pure Nix; Nix already provides the logic Lua would have added. See PLAN.md section 10 |
| XWayland       | **xwayland-satellite**, spawned on demand        | Maintained for years, used by niri. Smithay XWM kept as fallback only. See PLAN.md section 8 |
| Workspaces     | **configurable**: `global` or `per_output`       | extended in round 2 |
| IPC            | **yes**, a control socket + `msg` CLI, in v1     | Protocols stay the primary interface. See [ipc.md](ipc.md) |
| Blur           | **v1** (phase 6)                                 | ext-background-effect, dual-kawase |
| License        | **GPL-3.0-or-later**                             | |
| Default binds  | labwc defaults merged with your niri binds       | See [keybinds.md](keybinds.md) |
