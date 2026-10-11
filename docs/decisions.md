# Decisions

Log of review decisions on the plan. Newest first.

## Round 1

| topic          | decision                                         | notes |
| -------------- | ------------------------------------------------ | ----- |
| Name           | open, candidates in [names.md](names.md)         | "wl-hex" is a placeholder |
| Config format  | **TOML** primary, JSON accepted, no Lua          | Must be configurable from pure Nix; Nix already provides the logic Lua would have added. See PLAN.md section 10 |
| XWayland       | **xwayland-satellite**, spawned on demand        | Maintained for years, used by niri. Smithay XWM kept as fallback only. See PLAN.md section 8 |
| Workspaces     | **configurable**: `global` or `per_output`       | ext-workspace groups follow the mode |
| IPC            | **yes**, a control socket + `msg` CLI, in v1     | Protocols stay the primary interface. See [ipc.md](ipc.md) |
| Blur           | **v1** (phase 5)                                 | ext-background-effect, dual-kawase |
| License        | **GPL-3.0-or-later**                             | |
| Default binds  | labwc defaults merged with your niri binds       | See [keybinds.md](keybinds.md) |
