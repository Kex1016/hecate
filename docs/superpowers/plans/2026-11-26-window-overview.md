# Window overview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Meta+W opens a compositor-drawn overview that lays every window of the current workspace out in a grid, live, in the haiiro look; click or arrow keys + Return focus a window, an `x` closes one, typing filters.

**Architecture:** The overview lives in hecate itself (no shell, no capture protocol). A new `overview` module holds a pure grid layout, the open/closed state and its key and pointer handling. While it is open, `render_elements_for` draws each window's live surfaces through `RescaleRenderElement` into its slot, with a crisp haiiro frame drawn at slot size (the border code's frame half is split out for that). The transition is one `Animation` from `animations.overview`: `slide` lerps every window from where it is to its slot, `pixelate` cuts to the slots and sharpens them, `none` is a cut.

**Tech Stack:** Rust, Smithay (GLES renderer, `RescaleRenderElement`, `PixelCache`), hecate's test fixture (headless + llvmpipe render tests), Nix Home Manager module.

**Spec:** David's message in the "Plan the window overview" project thread (2026-11-26), quoted here because no separate spec doc exists:

> Since hecate has its own compositor, the cleanest approach is to render the overview there: on Meta+W it animates each window on the current workspace to a grid slot (like GNOME or macOS Exposé), with click or arrow keys to focus, an x to close, and type-to-filter. Doing it in the shell instead would need window-capture protocols for live thumbnails, which is more fragile. […] Two things to decide: whether it should also show other workspaces in a strip along the top, and whether the layout should pixelate or dither the way the rest of haiiro does.

Look: the haiiro design system (https://claude.ai/artifact/YMSijrDsHwzCfhFi4jDaSA), Shell section: double pine frames with the notch label, gold for "this one", love for destructive, IBM VGA 8x16 text, square corners.

**Decided (David, 2026-11-26):** workspace strip along the top, and the haiiro look pixelates (Task 7 sets `animations.overview` to the stepped pixelate). The strip is Task 7; haiiro defaults and docs move to Task 8.

## Global Constraints

- Repo: `Kex1016/hecate`, branch `claude/project-thread-149ugf` cut from `master` (`e73f802`); one PR to `master`, merged only after David approves.
- Commits: author and committer `Kex1016 <cakes@haiiro.moe>`; one brief conventional-commit subject line; no body, no co-authors, no trailers; no mention of AI.
- Never force-push.
- No shell (QML), hecate-greetd or cakeos changes. cakeos picks the new hecate up through its flake-update workflow after the merge.
- Bind: `"Mod+W" = "toggle_overview"` in the **base** binds (`config/defaults.rs`), so it works with or without the shell. `Mod+W` is unbound today in both base and shell binds.
- Action name `toggle_overview`; IPC event `OverviewChanged { open: bool }`; config section `[overview]`; animation `animations.overview`.
- Rust defaults keep hecate's non-haiiro look (`slide`, 300 ms, `emphasized_decel`); the haiiro values come from `nix/hm-module.nix` (`haiiroSettings`), and `settings` still wins over them.
- The grid holds the windows of each output's **active workspace**, excluding minimized windows; the strip above it shows that output's workspaces. Popups are not drawn in the overview.
- Switching workspace while the overview is open (strip click, PageUp/PageDown, a bind, IPC) keeps it open and shows the new workspace's grid.
- Every output shows its own grid at once; keyboard selection moves within the focused output's grid; clicks work on any output.
- Nothing opens while the session is locked; locking closes it at once.
- Run tests inside `nix develop` (Mesa llvmpipe for render tests). Never commit `haiiro-compositor/harness/headless-renderer.patch`.
- Screenshot for David: one headless capture of the open overview (recipe in project files `haiiro-compositor/harness/run.sh`), saved to project files `window-overview/overview.png`.

## Review Focus

1. **A window maps, unmaps or closes while the overview is open, including the selected one.** Expected: the grid relays out, selection moves to the next window in reading order (or none), no stale `Window` is drawn or focused. Test in Task 2: `window_closed_while_open_relayouts_and_moves_selection`.
2. **The screen locks while the overview is open** (idle lock, `Mod+L`, lid). Expected: overview closes at once with no animation, the lock surface gets the keyboard, typed keys never reach the filter. Test in Task 2: `locking_closes_the_overview`.
3. **Two outputs, one at fractional scale 1.25.** Expected: each grid fills its own output's usable area, slots land on whole logical pixels, a click on the second output picks a window there. Test in Task 6: `click_on_second_output_at_fractional_scale_focuses_its_window`.
4. **A fullscreen window on the workspace.** Expected: it gets a slot like any other, and the Top layer (the bar) is drawn during the overview (normally a fullscreen window hides it). Test in Task 4: `fullscreen_window_gets_a_slot_and_the_bar_shows`.
5. **Typing with modifiers, or a filter nothing matches.** Expected: `Ctrl`/`Mod`/`Alt` combos (other than `Mod+W`) do nothing to the filter, a character the font can't draw is still stored and filtered on, and Return with zero matches does nothing. Test in Task 5: `modified_keys_and_no_matches_leave_the_overview_alone`.

---

### Task 1: Grid layout

Pure geometry, no state. Model: sonnet, effort medium.

**Files:**
- Create: `crates/hecate/src/overview/mod.rs` (only `pub mod layout;` for now)
- Create: `crates/hecate/src/overview/layout.rs`
- Modify: `crates/hecate/src/main.rs` (add `mod overview;` after `mod overrides;`)

**Interfaces:**
- Consumes: `crate::render::border::Extents` (`top/right/bottom/left: i32`, `around`, `inside`), `crate::config::action::Direction`.
- Produces:
  - `pub struct Item { pub rect: Rectangle<i32, Logical>, pub extents: Extents }` (the window's content rect where it is now, and its frame extents)
  - `#[derive(Debug, Clone, Copy, PartialEq)] pub struct Slot { pub frame: Rectangle<i32, Logical>, pub content: Rectangle<i32, Logical>, pub scale: f64 }`
  - `pub fn grid(area: Rectangle<i32, Logical>, items: &[Item], gap: i32) -> Vec<Slot>`: same length and order as `items`; which item gets which cell follows `reading_order` of the items' current rects, so windows land near where they were.
  - `pub fn reading_order(rects: &[Rectangle<i32, Logical>], columns: usize) -> Vec<usize>`
  - `pub fn neighbour(slots: &[Slot], from: usize, dir: Direction, eligible: &[bool]) -> Option<usize>`

Algorithm `grid` must follow (the tests pin it): try every column count `c` in `1..=n`, `r = ceil(n / c)`; cell = `((area.w - gap*(c+1)) / c, (area.h - gap*(r+1)) / r)`; each item's `scale = min(1.0, (cell.w - ext.left - ext.right) / rect.w, (cell.h - ext.top - ext.bottom) / rect.h)`; keep the `c` with the largest summed scaled content area (ties: fewer columns). Content size = `rect.size * scale` rounded down; frame = `extents.around(content)`, centred in its cell. Rows are centred horizontally when the last row is short. Every coordinate is a whole logical pixel. `reading_order` sorts by centre y, chunks into rows of `columns`, sorts each row by centre x. `neighbour` picks the eligible slot whose frame centre lies in `dir` from `from`'s centre, nearest by `primary distance + 2 * cross distance`; `None` if there is none.

- [ ] **Step 1: Write the failing tests** (in `layout.rs`, `#[cfg(test)] mod tests`)

```rust
const EXT: Extents = Extents { top: 13, right: 5, bottom: 5, left: 5 };
fn at(x: i32, y: i32, w: i32, h: i32) -> Item { Item { rect: Rectangle::new((x, y).into(), (w, h).into()), extents: EXT } }
fn item(w: i32, h: i32) -> Item { at(0, 0, w, h) }
fn area() -> Rectangle<i32, Logical> { Rectangle::new((0, 0).into(), (1920, 1080).into()) }

#[test]
fn one_small_window_is_not_scaled_up() {
    let s = grid(area(), &[item(400, 300)], 24);
    assert_eq!(s[0].scale, 1.0);
    assert_eq!(s[0].content.size, (400, 300).into());
    assert_eq!(s[0].frame, EXT.around(s[0].content));
    // centred in the area
    assert_eq!(s[0].frame.loc.x + s[0].frame.size.w / 2, 960);
}

#[test]
fn four_equal_windows_make_a_two_by_two_grid() {
    let s = grid(area(), &[item(1920, 1080), item(1920, 1080), item(1920, 1080), item(1920, 1080)], 24);
    let xs: std::collections::BTreeSet<_> = s.iter().map(|s| s.frame.loc.x).collect();
    let ys: std::collections::BTreeSet<_> = s.iter().map(|s| s.frame.loc.y).collect();
    assert_eq!((xs.len(), ys.len()), (2, 2));
    let first = s[0].scale;
    assert!(s.iter().all(|x| (x.scale - first).abs() < 1e-9 && x.scale < 0.5));
}

#[test]
fn slots_never_overlap_and_stay_inside_the_area() {
    let items: Vec<_> = (0..7).map(|i| item(300 + 100 * i, 900 - 80 * i)).collect();
    let s = grid(area(), &items, 24);
    for (i, a) in s.iter().enumerate() {
        assert!(area().contains_rect(a.frame));
        for b in &s[i + 1..] { assert!(!a.frame.overlaps(b.frame)); }
    }
}

#[test]
fn empty_input_gives_no_slots() { assert!(grid(area(), &[], 24).is_empty()); }

#[test]
fn reading_order_is_rows_then_x() {
    let r = |x, y| Rectangle::new((x, y).into(), (100, 100).into());
    assert_eq!(reading_order(&[r(900, 0), r(0, 600), r(0, 0), r(900, 600)], 2), vec![2, 0, 1, 3]);
}

#[test]
fn neighbour_moves_by_direction_and_skips_ineligible() {
    // top-left, top-right, bottom-left, bottom-right: they keep those cells
    let s = grid(area(), &[at(0, 0, 900, 500), at(1000, 0, 900, 500), at(0, 600, 900, 500), at(1000, 600, 900, 500)], 24);
    assert!(s[0].frame.loc.x < s[1].frame.loc.x && s[0].frame.loc.y < s[2].frame.loc.y);
    assert_eq!(neighbour(&s, 0, Direction::Right, &[true; 4]), Some(1));
    assert_eq!(neighbour(&s, 0, Direction::Down, &[true; 4]), Some(2));
    assert_eq!(neighbour(&s, 0, Direction::Left, &[true; 4]), None);
    assert_eq!(neighbour(&s, 0, Direction::Right, &[true, false, true, true]), Some(3));
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop -c cargo test -p hecate overview::layout`
Expected: compile error, `grid` not found.

- [ ] **Step 3: Implement `grid`, `reading_order` and `neighbour` in `overview/layout.rs`** as described above.

- [ ] **Step 4: Run them to see them pass**

Run: `nix develop -c cargo test -p hecate overview::layout`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/hecate/src/main.rs crates/hecate/src/overview
git commit -m "feat(overview): add grid layout for the window overview"
```

---

### Task 2: Overview state, action, config and event

Model: sonnet, effort high (touches window lifecycle and lock).

**Files:**
- Modify: `crates/hecate/src/overview/mod.rs`
- Modify: `crates/hecate/src/state.rs` (field `pub overview: crate::overview::Overview` on `Hecate`, `Default` in the constructor near `identify_until: None`)
- Modify: `crates/hecate/src/config/action.rs` (`ToggleOverview` under `// Windows`, plus a parse case in `bare_and_table_actions`)
- Modify: `crates/hecate/src/actions.rs` (`Action::ToggleOverview => self.toggle_overview()`)
- Modify: `crates/hecate/src/config/mod.rs` (`pub overview: Overview` on `Config`; `pub overview: Anim` on `Animations`, default `Anim::new(AnimStyle::Slide, 1.0, 300, "emphasized_decel")`)
- Modify: `crates/hecate/src/config/defaults.rs` (`"Mod+W" = "toggle_overview"` after `"Mod+Q"`)
- Modify: `crates/hecate-ipc/src/lib.rs` (`Event::OverviewChanged { open: bool }`)
- Hook calls into: the window unmap/destroy path and the map path in `handlers/xdg_shell.rs` (wherever `wm.closing` snapshots are taken), workspace activation in `wm/ops.rs`, and lock activation (`LockState::Locked`).
- Test: `crates/hecate/src/tests/overview.rs` (register `mod overview;` in `tests/mod.rs`)

**Interfaces:**
- Consumes: Task 1's `Item`, `Slot`, `grid`; `Wm::usable_area`, `Wm::active_workspace`, `border::extents_for`, `Animation::start`, `State::activate_window`, `State::ipc_broadcast_event`.
- Produces:
  - Config: `#[serde(default, deny_unknown_fields)] pub struct Overview { pub gap: i32 /* 24 */, pub dim: f32 /* 0.35 */ }` in `config/mod.rs` (name it `OverviewConfig` if `Overview` clashes in scope).
  - `pub struct Entry { pub window: Window, pub slot: Slot, pub from: Rectangle<i32, Logical> /* content rect when it opened */ }`
  - `#[derive(Default)] pub struct Overview { pub open: bool, pub anim: Option<Animation>, pub filter: String, pub selected: Option<Window>, pub previous_focus: Option<Window>, pub per_output: HashMap<String, Vec<Entry>> }`
  - `impl Overview { pub fn progress(&self, now: Duration) -> f64; pub fn is_visible(&self, now: Duration) -> bool; pub fn matches(&self, w: &Window) -> bool; pub fn entries(&self, output: &Output) -> &[Entry] }`. `progress` is 1 when open and settled, 0 when closed; it runs 0→1 on open and 1→0 on close. Visible = open, or a close still animating.
  - `pub fn filter_matches(filter: &str, app_id: &str, title: &str) -> bool`: case-insensitive substring over `label::app_name(app_id)`, the raw `app_id` and `title`; empty filter matches all.
  - `impl State { pub fn open_overview(&mut self); pub fn close_overview(&mut self, focus: Option<Window>); pub fn toggle_overview(&mut self); pub fn relayout_overview(&mut self) }`. `open` is a no-op when locked or when no output's active workspace has a non-minimized window; it remembers `focused_window()` as `previous_focus` and selects it (else the first in reading order). `close(Some(w))` activates `w`; `close(None)` refocuses `previous_focus`. Both broadcast `OverviewChanged` and `queue_redraw_all`. Layout area per output: `usable_area` shrunk by `gap` on every side, with `label::NOTCH_HEIGHT + gap` more reserved at the top for the filter prompt.

- [ ] **Step 1: Write the failing tests** in `tests/overview.rs` (fixture: one 1920×1080 output, `Fixture::new()` so base binds are on; `Mod` is Super, keycode `125 + 8`, `W` is `17 + 8`)

```rust
#[test] fn mod_w_toggles_and_announces() {
    // map two windows; press Mod+W
    assert!(f.hecate().hecate.overview.open);
    assert_eq!(f.hecate().hecate.overview.per_output["headless-1"].len(), 2);
    // ipc event stream got OverviewChanged { open: true }; second Mod+W closes and sends open: false
}
#[test] fn opening_with_no_windows_does_nothing() { /* toggle_overview(); assert!(!open) */ }
#[test] fn minimized_windows_are_left_out() { /* 2 windows, minimize one, open; len() == 1 */ }
#[test] fn closing_without_choice_refocuses_the_previous_window() {
    // focus B, open, close_overview(None): focused() == B
}
#[test] fn window_closed_while_open_relayouts_and_moves_selection() {
    // windows A, B, C; open; selected = B; destroy B's toplevel; dispatch
    // per_output len == 2, selected == the window after B in reading order, no entry holds B
}
#[test] fn window_mapped_while_open_gets_a_slot() { /* len grows to 3 */ }
#[test] fn locking_closes_the_overview() {
    // open; lock_session(); assert!(!open && anim.is_none()); toggle_overview() while locked keeps it closed
}
#[test] fn switching_workspace_while_open_shows_its_windows() { /* 1 window on ws 1, 2 on ws 2; open; do_action(Workspace(2)); open && len() == 2 */ }
#[test] fn filter_matches_app_name_app_id_and_title() {
    assert!(filter_matches("fire", "org.mozilla.firefox", "Mozilla"));
    assert!(filter_matches("MOZ", "x", "Mozilla Firefox"));
    assert!(!filter_matches("kitty", "foot", "~"));
    assert!(filter_matches("", "foot", "~"));
}
```

Plus in `config/action.rs` tests: `assert_eq!(parse(r#"a = "toggle_overview""#), Action::ToggleOverview);`, and in `config/load.rs` tests: a config with `[overview]\ngap = 8\n[animations.overview]\nstyle = "pixelate"` parses, and `[overview]\nbogus = 1` is rejected.

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop -c cargo test -p hecate overview toggle_overview`
Expected: compile errors (`Action::ToggleOverview`, `overview` field missing).

- [ ] **Step 3: Implement** the config, action, event, `Overview` state and the four `State` methods; call `relayout_overview` from map/unmap and workspace activation, and `close_overview` from lock (locking sets `anim = None` so it is gone in the same frame). Also update the JSON schema snapshot if `nix/checks.nix` compares one (`hecate check` / `schemars`).

- [ ] **Step 4: Run them to see them pass**

Run: `nix develop -c cargo test -p hecate overview toggle_overview config::`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat(overview): add toggle_overview action, state and Mod+W bind"
```

---

### Task 3: Split the window frame from its body

A refactor so the overview can draw a crisp frame around scaled content. Model: sonnet, effort medium.

**Files:**
- Modify: `crates/hecate/src/render/border.rs:501` (`push_body`)
- Test: `crates/hecate/src/tests/haiiro.rs` (existing tests must pass unchanged) and one new test there

**Interfaces:**
- Produces: `pub fn push_frame(renderer: &mut GlesRenderer, out: &mut Vec<WindowElement>, window: &Window, geo: Rectangle<i32, Logical>, output_loc: Point<i32, Logical>, scale: Scale<f64>, alpha: f32, style: &BorderStyle)`: everything `push_body` draws after the surface elements (the `if !style.draw_border { return; }` part onward: notch label, band, lines). `push_body` becomes surfaces + `push_frame`.

- [ ] **Step 1: Write the failing test** `push_frame_draws_only_the_frame` in `tests/haiiro.rs`: with the haiiro border config and a mapped window, collect `push_frame` elements; assert none is `WindowElement::Surface`/`Clipped`, at least one is `WindowElement::Border`, and one is `WindowElement::Label` when `border.label.enable = true`.
- [ ] **Step 2: Run** `nix develop -c cargo test -p hecate push_frame` → FAIL, `push_frame` not found.
- [ ] **Step 3: Extract `push_frame`** from `push_body`; no behaviour change.
- [ ] **Step 4: Run** `nix develop -c cargo test -p hecate haiiro` → all pass, including every existing pixel test.
- [ ] **Step 5: Commit** `git commit -am "refactor(border): split the frame out of push_body"`

---

### Task 4: Draw the overview

Model: opus, effort high (render pipeline, damage, animation styles).

**Files:**
- Create: `crates/hecate/src/overview/render.rs` (`impl Hecate { pub(crate) fn push_overview(...) }`)
- Modify: `crates/hecate/src/render/mod.rs:426-503` (in `render_elements_for`: when `self.overview.is_visible(now)`, skip the normal window loop and the workspace-switch windows, call `push_overview` in their place, and always draw `Layer::Top` above it even with a fullscreen window)
- Modify: the redraw scheduling that keeps animations ticking (follow how `wm.closing` keeps frames coming) so an overview animation redraws until done
- Test: `crates/hecate/src/tests/overview.rs`

**Interfaces:**
- Consumes: Task 2 `Overview`, `Entry`, `progress`, `matches`; Task 3 `border::push_frame`; `border::style_for`, `border::push_popups` (not used: no popups), `push_scaled`/`RescaleRenderElement`, `pixelate::PixelCache::element`, `pixelate::cell_px`, `Animation::pixelate_cell`, `config.border.tint.gold`, `config.overview.dim`.
- Produces: `pub(crate) fn push_overview(&self, renderer: &mut GlesRenderer, out: &mut Vec<HecateElement>, output: &Output, now: Duration, markers: &mut Vec<BlurMarker>)`.

What each entry draws, topmost first: frame via `push_frame` at `slot.content` (style from `style_for(.., focused=false)`, colour `border.tint.gold` when it is `selected`); surfaces rendered at the window's real size with their top-left at `slot.content.loc`, wrapped in `RescaleRenderElement::from_element(e, slot.content.loc (physical), slot.scale)`. Non-matching entries draw at alpha `dim`. By style of `animations.overview`, with `p = progress(now)`:
- `slide`: content rect = lerp(`entry.from`, `slot.content`, `p`), scale = lerp(1, `slot.scale`, `p`); frame follows the content rect.
- `pixelate`: slots from the first frame; frame + scaled content go through the window's `PixelCache` (as `push_window` does) at `cell_px(anim.pixelate_cell(now, closing))`. Closing pixelates the slots out and then cuts to the normal desktop.
- `none` / animations disabled: cut.

- [ ] **Step 1: Write the failing render tests** (`if !f.enable_rendering() { return; }` first; output 400×300; `BG = #102030`; border config like `haiiro.rs`'s `BLOCKS` plus `[border.tint]\ngold = "#ffff00"`; windows via `map_shm_window` red 300×200 and blue 300×200)

```rust
#[test] fn open_overview_draws_each_window_in_its_slot() {
    // open; settle; render; for each entry: frame.assert_rgb(centre of slot.content, RED or BLUE)
    // and the old window position outside every slot shows BG
}
#[test] fn selected_slot_frame_is_gold() { /* pixel on selected slot's top line == [255,255,0]; other slot's == inactive */ }
#[test] fn non_matching_windows_are_dimmed() {
    // set filter to the blue window's app_id; red slot centre ≈ RED*0.35 + BG*0.65 (±2 per channel)
}
#[test] fn slide_is_halfway_at_half_time() {
    // manual_clock; animations.overview = { style = "slide", ms = 400, curve = "linear" }
    // open; advance 200; the red content's left edge is at round((from.x + slot.content.x) / 2) ±1
}
#[test] fn pixelate_open_starts_in_blocks() {
    // animations.overview = { style = "pixelate", ms = 400, curve = "stepped", cell = 16, fade = false }
    // open; render at t=0: the 16×16 block containing the red slot's top-left content pixel is uniform
    // advance 400: content is sharp (pixel just inside the frame is pure RED)
}
#[test] fn closing_returns_windows_to_their_places() { /* close; settle; window rect centre is RED again */ }
#[test] fn fullscreen_window_gets_a_slot_and_the_bar_shows() {
    // a Top-layer 400×20 layer anchored top, exclusive zone 20, green; fullscreen the red window; open
    // pixel (200, 10) is green; the red window has a slot below y = 20
}
```

- [ ] **Step 2: Run** `nix develop -c cargo test -p hecate tests::overview` → the new tests FAIL (windows still drawn at their real places).
- [ ] **Step 3: Implement `push_overview`** and the `render_elements_for` hook.
- [ ] **Step 4: Run** `nix develop -c cargo test -p hecate tests::overview tests::haiiro tests::render tests::animations` → all pass.
- [ ] **Step 5: Commit** `git commit -am "feat(overview): draw windows in a live grid"` (with `git add` of `overview/render.rs`)

---

### Task 5: Keyboard: select, focus, filter

Model: sonnet, effort high.

**Files:**
- Create: `crates/hecate/src/overview/input.rs`
- Modify: `crates/hecate/src/input/mod.rs:36` (`KeyResult::Overview(Keysym, Modifiers, Option<char>)`) and the intercept closure in `handle_key`: when the overview is open, every **press** returns `Intercept(KeyResult::Overview(..))` except a bind with `allow_when_locked` (so `Mod+L` still locks) and VT switches; releases of keys pressed while open are swallowed. `Mod+W` reaches the overview as a key, not through the bind table.
- Modify: `crates/hecate/src/overview/render.rs` (the filter prompt)
- Test: `crates/hecate/src/tests/overview.rs`

**Interfaces:**
- Consumes: Task 1 `neighbour`; Task 2 state and `close_overview`, `matches`; `label::layout`, `label::rasterize_line`, `label::NOTCH_HEIGHT`; `handle.modified_sym().key_char()` for the typed character.
- Produces: `impl State { pub(crate) fn overview_key(&mut self, sym: Keysym, mods: Modifiers, text: Option<char>) }`.

Keys: `Escape` and `Mod+W` close with `None`; `Return`/`KP_Enter` close with the selected window if it matches, else do nothing; arrows move with `neighbour` over matching entries of the focused output; `BackSpace` pops a character; `Ctrl+U` clears the filter; any other key with a printable `text` and no `Ctrl`/`Alt`/`Mod` appends it. After a filter change, if the selection no longer matches, select the first match in reading order (or `None`). Prompt: when `filter` is non-empty, `"> {filter}_"` laid out with `label::layout` (cut with "…" to the output width minus `2*gap`), rasterized with `border.label.color` on `border.background`, centred horizontally in the reserved top row. A character with no glyph is drawn as `?` but kept in `filter`.

- [ ] **Step 1: Write the failing tests** (keycodes xkb = evdev + 8: Esc 1, Return 28, BackSpace 14, Left 105, Right 106, Up 103, Down 108, F 33, I 23, R 19, Ctrl 29, U 22, Super 125, W 17)

```rust
#[test] fn arrows_move_the_selection() { /* 4 windows, open, select slot 0; Right → slot 1; Down → slot 3 */ }
#[test] fn return_focuses_the_selected_window_and_closes() { /* Right, Return; !open; focused() == slot 1's window */ }
#[test] fn escape_closes_and_keeps_focus() { /* focused unchanged */ }
#[test] fn typing_filters_and_backspace_undoes() {
    // apps "firefox", "foot", "kitty": type "fi" → only firefox matches, selected == firefox
    // BackSpace → filter == "f", foot matches too; Ctrl+U → filter empty
}
#[test] fn keys_do_not_reach_the_client_while_open() {
    // open; type "f"; the focused client's recent_keyboard_events has no key event for it
}
#[test] fn modified_keys_and_no_matches_leave_the_overview_alone() {
    // Ctrl+F → filter still ""; type "zzz" → no match, selected None; Return → still open
}
#[test] fn lock_bind_still_works_while_open() { /* bind Mod+L lock (allow_when_locked) → locked and overview closed */ }
#[test] fn filter_prompt_is_drawn() { /* render test: type "fi"; a pixel of border.label.color inside the prompt row */ }
```

- [ ] **Step 2: Run** `nix develop -c cargo test -p hecate tests::overview` → new tests FAIL.
- [ ] **Step 3: Implement** `overview_key`, the `KeyResult::Overview` path and the prompt.
- [ ] **Step 4: Run** `nix develop -c cargo test -p hecate tests::overview tests::input` → all pass.
- [ ] **Step 5: Commit** `git commit -m "feat(overview): select, focus and filter windows from the keyboard"`

---

### Task 6: Pointer: hover, click, close button

Model: sonnet, effort high.

**Files:**
- Modify: `crates/hecate/src/overview/input.rs`
- Modify: `crates/hecate/src/input/mod.rs` (`on_pointer_button`: while the overview is open, a press goes to `overview_click` and is suppressed like a pick; `on_pointer_motion`/`_absolute`: while open, `overview_hover` and pointer focus `None`, so clients get a leave and nothing else)
- Modify: `crates/hecate/src/overview/render.rs` (the close button)
- Test: `crates/hecate/src/tests/overview.rs`

**Interfaces:**
- Produces:
  - `pub enum Hit { Slot(Window), Close(Window), Empty }`
  - `impl State { pub(crate) fn overview_hit(&self, pos: Point<f64, Logical>) -> Hit; pub(crate) fn overview_click(&mut self, pos: Point<f64, Logical>, button: u32); pub(crate) fn overview_hover(&mut self, pos: Point<f64, Logical>) }`
  - `pub fn close_button(frame: Rectangle<i32, Logical>) -> Rectangle<i32, Logical>` in `overview/layout.rs`: 16×16 at `(frame.right() - 4 - 16, frame.top())`, on the top line like a notch.

Behaviour: hovering a matching slot selects it. Left click on a slot closes with that window; on its close button sends `xdg_toplevel.close` (via `Action::Close` with the window as target) and keeps the overview open; on empty space closes with `None`. Middle click on a slot also closes that window. Other buttons are swallowed. Clicks on non-matching slots count as empty. The close button is drawn only on the selected slot: an `x` from `label::layout("x", ..)`, `border.background` text on `border.tint.love`.

- [ ] **Step 1: Write the failing tests** (call `overview_click`/`overview_hover` directly, as `lock.rs` calls `focus_lock_surface_at`)

```rust
#[test] fn hover_selects() { /* overview_hover(centre of slot 1) → selected == slot 1 */ }
#[test] fn click_slot_focuses_it() { /* BTN_LEFT on slot 1 centre → !open, focused == slot 1 */ }
#[test] fn click_close_button_asks_the_window_to_close() {
    // overview_click(centre of close_button(slot0.frame)) → client window close_requested, overview still open
}
#[test] fn middle_click_closes_a_window() { /* BTN_MIDDLE (0x112) on slot 0 → close_requested */ }
#[test] fn click_on_empty_space_closes_the_overview() { /* (5, 5) → !open, focus unchanged */ }
#[test] fn click_on_second_output_at_fractional_scale_focuses_its_window() {
    // outputs headless-1 1920×1080 and headless-2 1920×1080 at scale 1.25 (logical 1536×864, right of 1)
    // one window on each; open; every slot of headless-2 lies inside its usable area with integer coords
    // click its slot centre → focused == that window, focused_output == headless-2
}
#[test] fn close_button_is_drawn_on_the_selected_slot() { /* render: love pixel inside close_button(selected.frame) */ }
```

- [ ] **Step 2: Run** `nix develop -c cargo test -p hecate tests::overview` → new tests FAIL.
- [ ] **Step 3: Implement** hit testing, click and hover, the input hooks and the close button.
- [ ] **Step 4: Run** `nix develop -c cargo test -p hecate` → whole crate passes (input paths changed for everything).
- [ ] **Step 5: Commit** `git commit -m "feat(overview): pick and close windows with the pointer"`

---

### Task 7: Workspace strip

A row of live workspace thumbnails above the grid. Model: opus, effort high (second scaled scene, drag and drop).

**Files:**
- Create: `crates/hecate/src/overview/strip.rs` (layout + drawing + hit testing)
- Modify: `crates/hecate/src/config/mod.rs` (`strip: bool` on the overview config, default `true`)
- Modify: `crates/hecate/src/overview/mod.rs` (the grid's reserved top area grows by the strip's height when `strip` is on; `Overview` gains `pub drag: Option<Drag>`)
- Modify: `crates/hecate/src/overview/render.rs`, `crates/hecate/src/overview/input.rs`, `crates/hecate/src/input/mod.rs` (release and motion while dragging)
- Test: `crates/hecate/src/tests/overview.rs`

**Interfaces:**
- Consumes: Tasks 1–6; `Wm` workspace list for an output (fixed: `workspaces.count`; dynamic: the existing ones plus the trailing empty one), `Action::Workspace`, `Action::MoveToWorkspace`, `border::push_frame`, `RescaleRenderElement`.
- Produces:
  - `pub struct Thumb { pub workspace: u64, pub index: u32, pub frame: Rectangle<i32, Logical>, pub content: Rectangle<i32, Logical>, pub scale: f64 }`
  - `pub fn strip(area: Rectangle<i32, Logical>, output: Size<i32, Logical>, count: usize, gap: i32, extents: Extents) -> Vec<Thumb>` (pure; `workspace`/`index` filled by the caller): height `area.h * 0.12` rounded, each content rect has the output's aspect ratio, the row is centred, and it shrinks (never overlaps, never leaves `area`) when there are many workspaces.
  - `pub struct Drag { pub window: Window, pub start: Point<f64, Logical>, pub pos: Point<f64, Logical>, pub active: bool }`
  - `Hit` gains `Workspace(u64)`.

Drawing: each thumb draws that workspace's non-minimized windows at their real positions, scaled by `scale` into `content` (one `RescaleRenderElement` per surface, origin `content.loc`), over a `border.background` fill, inside a frame from `push_frame` whose notch label is the workspace's number (or name). The active workspace's frame is gold (`border.tint.gold`), the drop target under a drag is foam (`border.tint.foam`). Thumbs pixelate in with the grid.

Input: left click on a thumb switches to that workspace (overview stays open, Task 2); `Page_Up`/`Page_Down` switch to prev/next; `Shift+Page_Up`/`Shift+Page_Down` move the selected window there. A press on a slot that moves more than 8 logical px before release becomes a drag: the window's scaled content follows the pointer at its slot scale; releasing over a thumb runs `MoveToWorkspace` for it (overview stays on the current workspace, the grid relays out); releasing anywhere else drops it back with no change, and no click fires.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn strip_fits_and_keeps_the_output_aspect() {
    let t = strip(Rectangle::new((0, 0).into(), (1920, 1080).into()), (1920, 1080).into(), 9, 24, EXT);
    assert_eq!(t.len(), 9);
    for x in &t { assert!((x.content.size.w as f64 / x.content.size.h as f64 - 16. / 9.).abs() < 0.05); }
    for w in t.windows(2) { assert!(w[0].frame.right() < w[1].frame.left()); }
    assert!(t.iter().all(|x| x.frame.bottom() <= 130));
}
#[test] fn grid_moves_below_the_strip() { /* open; every slot.frame.top() > the strip's bottom; with strip = false it is not */ }
#[test] fn clicking_a_thumb_switches_and_stays_open() { /* overview_click(thumb 2 centre) → active ws 2, open, grid holds ws 2's windows */ }
#[test] fn page_down_switches_and_shift_page_down_moves_the_selection() { /* Next → ws 2; Shift+Prior → selected window now on ws 1 */ }
#[test] fn dragging_a_window_onto_a_thumb_moves_it() {
    // press on slot 0, hover 20 px away, then over thumb 3, release → window on ws 3, still open, grid len drops by 1, no focus change
}
#[test] fn a_short_press_is_still_a_click() { /* press + move 4 px + release on slot → focuses it (Task 6 behaviour) */ }
#[test] fn dropping_outside_a_thumb_changes_nothing() { /* release over empty space → same workspace, overview open */ }
#[test] fn thumbs_show_their_windows() { /* render: centre of ws 2's red window, mapped through thumb 2's scale, is RED; active thumb's top line is gold */ }
```

- [ ] **Step 2: Run** `nix develop -c cargo test -p hecate tests::overview` → new tests FAIL.
- [ ] **Step 3: Implement** `strip`, the thumbs' drawing, `Hit::Workspace`, the keys and the drag.
- [ ] **Step 4: Run** `nix develop -c cargo test -p hecate` → whole crate passes.
- [ ] **Step 5: Commit** `git commit -m "feat(overview): add a workspace strip with drag to move"` (with `git add crates/hecate/src/overview/strip.rs`)

---

### Task 8: haiiro defaults, docs and the screenshot

Model: haiku, effort medium (Nix value + docs text); the screenshot step needs the harness, so give it to sonnet if haiku can't run it.

**Files:**
- Modify: `nix/hm-module.nix` (`haiiroSettings.animations.overview = pixelate;`; `overview.gap = 24` stays the Rust default)
- Modify: `docs/config.md` (`[overview]` keys `gap`, `dim`, `strip`), `docs/animations.md` (`animations.overview` and what each style does there), `docs/keybinds.md` (base table: `Mod+W` → `toggle_overview`; drop "`Mod+W`" from the end-4 launcher sentence), `docs/ipc.md` (`OverviewChanged`), the strip and its keys (Task 7) in `docs/keybinds.md`, `CHANGELOG.md` (one entry under the unreleased heading)
- Create in project files: `window-overview/overview.png`

- [ ] **Step 1: Set the haiiro default** and check the module still evaluates: `nix eval .#checks.x86_64-linux --apply builtins.attrNames` lists the checks; `nix flake check` passes.
- [ ] **Step 2: Write the docs** listed above; every key name must match Task 2 exactly.
- [ ] **Step 3: Take the screenshot** with the headless renderer patch applied locally (`haiiro-compositor/harness/run.sh`, three kitty windows on workspace 1 and one on workspace 2 so the strip shows both, `hecate msg action toggle_overview`), save `window-overview/overview.png` in project files, then revert the patch (`git status` shows only the intended files).
- [ ] **Step 4: Run the full checks**: `nix develop -c cargo clippy --all-targets -- -D warnings`, `nix develop -c cargo test`, `nix flake check` → all green.
- [ ] **Step 5: Commit** `git commit -m "docs(overview): document the overview and give it the haiiro look"` (code and docs may be two commits: `feat(hm): pixelate the overview in the haiiro look` first)

Then push the branch, open the PR to `master` (attribution header, assign and request review from `Kex1016`), and post the screenshot in the thread.
