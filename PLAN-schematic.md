# cabin schematic panel — plan

Written 2026-11-18, against `games/cabin/` at commit `566c411` plus the uncommitted per-train delay change (70 tests, verify scripts 37 + 75 + 36 + 61 passed).
Method: a Codex review (read-only, `codex exec`) of the B6 screenshot and `crates/web/src/lib.rs`, saved in the session scratchpad as `codex-design-check.md`. `Panel` and every geometry call site in `crates/web/src/lib.rs` re-read; `data/ndls.yard` counted (273 nodes, 323 edges: 163 running, 16 siding, 144 yard; 148 points, 15 crossings, 16 platforms, 12 portals, 44 signals). Two Python prototypes of the layout run against `data/ndls.yard`, one rendered to PNG at 1920 × 1080. One fetch of an Indian Railways manual (it returned an HTML page, not the PDF).
**Built: S0 to S4, 2026-11-18 to 2026-11-19.** Corrected 2026-11-19: until then this line said nothing below was implemented. The "change" and "built" notes under each slice record what differs from the steps.

## Goal

**The panel draws NDLS the way a control-room screen does: straight horizontal tracks on fixed rows, 45° crossovers, and Indian Railways indication colours.** Only the drawing and the click targets change. The sim, its rules and its 70 tests stay as they are.

## Part A — findings, ranked

### A1 · DESIGN RISK (high): the panel draws geography, magnified six times

`Panel::map` (`crates/web/src/lib.rs:174`) turns the yard to its long axis, then multiplies each point's distance from a smoothed centreline by `PANEL_STRETCH` (6.0, `crates/sim/src/consts.rs:15`). `map_poly` keeps every vertex of the OpenStreetMap polyline. A 1 m bend becomes about 3 px at the B2 view (0.52 px/m), so every survey wobble shows, and throats become steep curves. That is the "squiggly" look.
**Fix:** replace `Panel` with a schematic layout (Part B, S1 and S2). A smaller stretch or more smoothing still draws geography, only flatter.

### A2 · MODEL GAP (high): snapping geography to a grid does not work

Prototype 1 rounded each node's cross-track offset to 5 m rows and drew each edge as a horizontal run plus one 45° jog. Result on all 323 edges: **117 overlapping horizontal runs, 72 edges too steep for 45°, and all 16 platform edges on slopes**, because a platform edge runs on into the throat (P16 drifts 81 m across in 525 m). Ordered by their midpoints, the platforms read 2, 1, 4, 5, 6, 3, 7 … across, which is wrong.
**Fix:** lay out by topology, not by distance (A3).

### A3 · DESIGN RISK (medium): the topology layout works on the prototype, with 3 false crossings left

Prototype 2 joins each point's stem and normal leg, each crossing's two straight pairs and each plain two-edge node into **through-lines**. One through-line is one horizontal row. Lines are ordered across by geography wherever two overlap along the axis, and each line goes to the lowest free row above every line it must clear. A reverse leg is a 45° jog into the point. An x axis that keeps node order and gives each jog its room fixes the slope.

| Measure | Running only | Running + siding |
|---|---|---|
| Through-lines | 42 | 46 |
| Rows | 22 | 25 |
| Ordering cycles | 0 | 0 |
| Platforms in order P1 to P16 across | yes | yes |
| Width against true length (2,256 m) | 1.22× | 1.49× |
| Drawn crossings away from any node (false crossings) | **3** | **7** |
| Row pitch when the whole yard fits 1800 × 1000 px | 19.5 px | 16.1 px |

With all 144 yard edges the order has a cycle and needs 65 rows, so yard tracks are not laid out (B, S1 step 6). The 3 or 7 false crossings are long diagonals that cross a row with no diamond. Some may be real flyovers; the yard file cannot tell. **This is the least certain part, so it is slice S1, and it has a stop condition.**

### A4 · INCONSISTENCY (medium): the colour rule forbids the red that a real panel uses

`PLAN-cabin.md § Design` keeps red, yellow and green inside `draw_heads` only, and `tools/verify_b8.py:318-325` fails the build if an aspect hex appears anywhere else. Indian Railways panels show occupied track in red (A5). Your instruction in this session ("look like an actual railway control room screen") ranks above the B-plan design, so the rule changes: indication colours live in one named palette block, and the grep checks that block instead.

### A5 · MODEL GAP (medium): the indication colours are unconfirmed

Codex cited the Indian Railways track-circuit chapter (2023) and the Signal Engineering Manual §7.172. It says: track unlit when clear, white for a set and locked route, red for occupied, and occupied overrides the route colour. The fetch of the first source returned an HTML page, and `rails/research.md` has no panel rows. **Until S0 confirms them, each colour is tagged `assumed` in the "what is real" panel.**

### A6 · COSMETIC (low): trains are white blocks, not lit track circuits

`draw` strokes each train body 7 px wide in ink (`lib.rs:1146-1168`). A real panel lights the whole track section red and prints the train number in a describer box. The sim's track sections are edges (`Refusal` text: "holds track section 258"), and `World::occupancy()` gives each train's spans, so the web crate can colour each occupied edge without a sim change.

### A7 · COSMETIC (low): points show no lie

Nothing draws a point's position. `World::lie(k)` already returns it. Real panels show normal or reverse at the switch.

### A8 · NOISE (low): five things die with `Panel`

`PANEL_STRETCH` (and its `assumed` tag), `App::stretch()`, the footer text "not to scale across the tracks (spacing ×6)" (`web/shell.html:762`), the "Panel" row of the "what is real" panel (`lib.rs:489`), and the web unit test that platforms lie along the axis (`lib.rs:1255-1264`). README says "21 constants are tagged"; it becomes 20.

## Global constraints

- `crates/sim` behaviour does not change. The one sim edit is removing `PANEL_STRETCH` from `consts.rs`. `cargo test --release --workspace` passes: 70 tests at the start, 70 + the new `schematic` tests at the end.
- One offline HTML file, no new crate, no new Python package, no network request.
- Both themes. Dark is the control-room look; light keeps the same roles with contrast-safe hues.
- 0 signal pairs under 44 px at the default view at 1920 px and 1500 px (`verify_b6.py`).
- Timestamped backup of `web/shell.html` before it is edited. No `alert()` or `confirm()`. No commit without your yes.
- Run `./build.sh` after the last source change and before any verify script.

## Part B — the build

### Files

| File | Change |
|---|---|
| `crates/web/src/schematic.rs` | **New.** `Schematic::new(&Yard) -> Schematic`: through-lines, rows, x axis, one polyline per edge, yard stubs. Unit tests hold the S1 gates |
| `crates/web/src/lib.rs` | `Panel`, `densify`, `SAMPLE_M`, `BIN_M`, `SMOOTH_BINS`, `DENSIFY_M` removed. `App::new`, `portal_xy`, the label code and the train drawing read the schematic. Indications in `draw` |
| `crates/sim/src/consts.rs` | `PANEL_STRETCH` removed |
| `web/shell.html` | Footer text; palette roles for the panel in both themes |
| `tools/verify_b8.py` | Colour-rule check reads the palette block |
| `tools/verify_b9.py` | **New.** The schematic gates in the browser (S2, S3) |
| `research/panel.md` | **New.** Indication colours, one row each, with a source id or `assumed` |
| `README.md`, `PLAN-cabin.md`, `tools/README.md`, `md/STATE-projects.md` | S4 |

### Interfaces the slices share

```rust
// crates/web/src/schematic.rs
pub struct Schematic {
    /// Panel position of every node the layout draws; None for yard-only nodes.
    pub node: Vec<Option<(f64, f64)>>,
    /// One polyline per edge in panel metres; empty for a yard edge.
    pub shape: Vec<Vec<(f64, f64)>>,
    /// Short stubs where a yard edge leaves a drawn node.
    pub stubs: Vec<[(f64, f64); 2]>,
    pub rows: usize,
}
impl Schematic {
    pub fn new(yard: &Yard) -> Schematic;
    /// Drawn crossings away from any node: (edge, edge). The S1 gate.
    pub fn false_crossings(&self) -> Vec<(usize, usize)>;
}
```

`at_metres(&shape[e], e.len, m)` and `piece(&shape[e], e.len, lo, hi)` already map the sim's metres onto any polyline by fraction, so signal heads and trains follow the schematic with no new function.

### Slices, each run before the next

| # | Slice | Done when |
|---|---|---|
| S0 | **Confirm the colours.** Fetch the two sources Codex cited with `obscura` (retry once with `--stealth`, and say so). Write `research/panel.md`: track clear, route set, occupied, point normal and reverse, signal ON, each with a source id or `assumed` | Every row has a source id or `assumed`. Rows that differ from A5 are listed for you |
| S1 | **The layout, tests first.** `schematic.rs` and its unit tests | The gates below pass and the numbers print. **Stop** if false crossings exceed 0 after step 5: list each pair with the two OpenStreetMap way ids for you to decide (flyover, or reorder) |
| S2 | **Draw it.** `App::new` takes `Schematic`; heads, trains, portals, labels and hit-testing read it; `Panel` and A8's five items go | `cargo test` passes; `verify_b2.py`, `verify_b6.py` (44 px), `verify_b7.py`, `verify_b8.py` pass; `verify_b9.py` checks the drawn routes; screenshots at 1920 and 1500 px in both themes are shown to you |
| S3 | **Indications.** Track, route, overlap, occupied, describer, point lie, from `research/panel.md` | `verify_b9.py`: each state's colour read from canvas pixels at a known edge; `verify_b8.py`'s colour rule passes on the palette block |
| S4 | **Docs.** README sections "What is assumed" and "What is not right yet", `PLAN-cabin.md` note, `tools/README.md` rows, `STATE-projects.md` row | README commands run as written and their output quoted |

### S1 — steps

- [ ] **1. Write the failing tests** in `schematic.rs`, against the baked yard (`Yard::parse(cabin_sim::NDLS)`):
  ```rust
  #[test] fn every_platform_is_one_horizontal_run_in_order() // P1..P16 rows strictly increase; row = y of the flat run (changed 2026-11-18, see below)
  #[test] fn every_jog_is_45_degrees_or_flat()               // each segment: |dy| == 0 or |dy| == |dx| (1e-6)
  #[test] fn edges_meet_at_their_nodes()                     // shape[e] starts at node[a], ends at node[b]
  #[test] fn every_route_draws_unbroken()                    // for each route, path + overlap shapes chain end to end
  #[test] fn no_two_runs_share_a_row_where_they_overlap()    // horizontal runs on one row are disjoint, 1 m slack
  #[test] fn each_crossing_draws_as_an_x()                   // both straight pairs pass through the crossing node
  #[test] fn false_crossings_are_none()                      // prints the list; the S1 stop condition
  ```
  Run `cargo test -p cabin-web` and confirm each fails on `todo!()`.
- [ ] **2. Through-lines.** A union-find over edges of kind running or siding: join a point's stem and normal, and the two edges at a node of degree 2. **Not** through a crossing (changed 2026-11-18, see below). A one-edge line with both ends on reverse legs is a **crossover**: no row. A platform edge is never a crossover (the prototype first lost P2 this way).
- [ ] **3. Rows.** Rotate as `Panel::new` does now: the platform-direction axis and the median centreline, moved into `schematic.rs`. They now give only order, never position. For each pair of lines that overlap by 20 m or more along the axis, sample 9 places; the line with the larger offset at most of them goes above. Topologically sort; a cycle is a test failure with the lines named. Give each line the lowest row above every line it must clear, with no other line on that row within 40 m along the axis.
- [ ] **4. x axis.** Sort drawn nodes by `u`. Walk them in order: `x[n] = max(x[prev] + ALONG * (u[n] - u[prev]), x[a] + |row[a] - row[n]| * ROW_M + JOG_PAD_M)` over every edge `a`–`n` that jogs. Constants at the top of `schematic.rs`, each with its reason: `ROW_M = 30.0`, `ALONG = 0.35` (prototype: 1.22× true width), `JOG_PAD_M = 8.0`. These are drawing knobs, not model claims, so they take no `solid`/`assumed` tag.
- [ ] **5. Shapes and the false-crossing pass.** Each edge: flat, or one 45° jog placed at the end that meets the reverse leg, else at its middle. Then try each pair of adjacent rows' line sets swapped where the swap keeps step 3's order; keep a swap that lowers `false_crossings().len()`. Stop when a full pass changes nothing.
- [ ] **5a. (added 2026-11-18)** The fixes that replaced the swap pass are in "S1 change of 2026-11-18" below.
- [ ] **6. Yard stubs.** Each yard edge that meets a drawn node gets one stub, `ROW_M * 0.6` long at 45°, on the side its first polyline segment leaves toward. 144 yard edges are not laid out (A3).
- [ ] **7. Run** `env CARGO_TARGET_DIR=$HOME/.cache/cabin-target cargo test --release -p cabin-web -- --nocapture`. Quote: rows, lines, crossovers, width against true length, false crossings and their way ids.

### S1 change of 2026-11-18

**Approved in chat on 2026-11-18 ("change the plan and build it yourself") after two Codex passes stopped at 23 and 22 false crossings.** Every change below was run against `data/ndls.yard`; the numbers are from `cargo test --release -p cabin-web -- --nocapture`.

| Change | Why | Effect on NDLS |
|---|---|---|
| **Platform row is the y of its flat run, not of its end node.** The flat run covers at least half the platform; a jog is allowed only at an end where the platform is a point's reverse leg | P3 is the normal and P4 the reverse leg of the point at node 91 (P9 and P10 at node 215). One node has one position, so P4 must leave it at 45°. The old test could never pass | P1..P16 on 16 rows, in order |
| **Each jog picks where to change rows.** Preferred at the foreign end (the end whose node row comes from another line: a turnout's reverse leg, the far side of a diamond); else the x that crosses and overlies no drawn track. Short jogs first, then a second pass with every shape drawn | A jog at a fixed end or the middle crossed tracks on the rows between | 22 → 7 false crossings |
| **Lines are not joined through a diamond.** | Two tracks that cross at a diamond swap sides; one line on one row cannot cross another | 7 → 2; rows 27 → 33, lines 47 → 67 |
| **A crossover fixes the side of its far line**, over the majority vote, if no cycle follows | The vote samples the whole shared span. The line at node 187 runs below line 129 for 700 m, yet crossover 107 (way 77601829) joins it from above beyond 129's end | 2 → 0 |
| **A diamond or line end takes the row of its longest line**, and the platform rule skips reverse legs | A 20 m edge between two diamonds (edge 120 at node 268) owned the node and overlay edge 305 | Overlap test passes |
| **No 8 m pad on an edge foreign at both ends; a jogging platform gets twice its jog in width** | The pad left a flat that lay over the owning track; P6 (435 m, drawn 222 m) was half jog | All 7 gates pass |

**Removed:** the adjacent-row swap pass (step 5) and a later any-row search. Neither removed a false crossing once the changes above were in, and the any-row search cost 8 s against 0.12 s.

**Result:** rows 33, lines 67, crossovers 6, width 4234 m against a true length of 2256 m (**1.88×, not the prototype's 1.22×**: the jogs need room). False crossings 0. The schematic is wider than geography; S2 fits it to the screen, so this costs zoom, not correctness. The Python prototype's "10 false crossings" was not comparable: it drew each edge's end at its own line's row, so edges did not meet at their nodes.

### S2 — steps

- [ ] **1.** Back up `web/shell.html` as `web/shell.backup-YYYYMMDD-HHMMSS.html`.
- [ ] **2.** In `App::new`, `shapes = schematic.shape.clone()`; `sig_path` uses `at_metres(&shapes[s.edge], …)` and takes the direction from the shape, not from `panel.map(p + d)`.
- [ ] **3.** `portal_xy` and the portal label read `schematic.node[p.node]`. Train bodies use `piece(&shapes[sp.edge], …)`. `bounds` reads `shapes` as now.
- [ ] **4.** Remove `Panel`, `densify`, its four constants, `App::stretch()`, `PANEL_STRETCH`, the footer text and the "Panel" row. Replace the row with "Panel · schematic · Track order and connections are the yard's; lengths and angles are drawn, not measured."
- [ ] **5.** `tools/verify_b9.py`: real clicks set 3 routes on seed 1; for each, sample canvas pixels along the drawn path and require the route colour from the entry head to the exit with no gap over 2 px; screenshots in both themes at 1920 and 1500 px.
- [ ] **6.** `./build.sh`, then all five verify scripts and `cargo test --release --workspace`. Show you the screenshots before S3.

### S2 changes of 2026-11-18

**Made after the first S2 screenshots, on "make the fixes".** The first S2 build drew the station with platforms 150 to 250 m wide inside a 4234 m drawing, platform ends that meet at one place in the yard 300 m apart, and platform labels on the jog corner beside the Starter's name.

| Change | Why |
|---|---|
| **The station area is drawn at true length** (`ALONG_STATION = 1.0` between the first and last platform end; `ALONG = 0.35` in the throats) | A panel is read at the platforms. At 0.35 throughout, P6 (435 m) drew 222 m wide while one 9-row jog drew 270 m |
| **Nodes within 6 m along the axis share one x** (`COLUMN_M`), unless an edge joins them | x must rise with u, so one jog's width needs moved every later node. Platform ends 2 m apart in the yard drew up to 300 m apart; now the platforms line up as one block |
| **Platform labels sit mid flat run**, not on the middle vertex | On a jogging platform the middle vertex is the jog corner, where the Starter's head and name sit |
| **`verify_b9.py` samples along the drawn route**, read from a new read-only hook `App::route_screen(id)`, every 1 px; a gap is more than 2 px of samples with no `--fg` pixel within 1 px. A route not set must fail the same sampler | Codex's first version searched all white pixels near both heads and needed both heads on screen; no Home is on the default view. It set 0 routes |

**Still true, by your 44 px rule:** rows are 15 px apart at 1500 px and 20 px at 1920 px, so heads on neighbouring tracks still slide back along their track with a stalk to the post. Rows 45 px apart would remove the stalks only at about twice the zoom.

### S3 — steps

- [x] **1.** One palette block in `draw`, read from CSS custom properties on the canvas (`--trk`, `--route`, `--occ`, `--asp-r`, `--asp-y`, `--asp-g`), so both themes come from `shell.html`. Dark: track `#3a3a3c` (unlit), route `#ffffff`, occupied `#e5484d`. Light: track `#c7c7cc`, route `#000000`, occupied `#b3261e`. Contrast measured against the canvas background and quoted.
- [x] **2.** Draw order: track, then set routes (path solid, overlap dashed), then occupied edges whole in red, which overrides the route colour (A5). The 7 px ink train block goes. Each train's number sits in a small boxed describer above its head, the box in `--route`.
- [x] **3.** Point lie: at each point node, the leg not lying is broken for its first 6 px from the switch. A point inside a set route shows its lie the same way.
- [x] **4.** `verify_b8.py`'s colour rule allows the hex values in the palette block and in the `shell.html` theme blocks only.
- [x] **5.** `verify_b9.py` gains: an occupied edge reads red at its middle pixel; a set route reads white (dark) or black (light); a broken leg shows background pixels within 6 px of its switch.

### S3 and S4 built 2026-11-19

**Built as written, with three changes.** Files: `crates/web/src/lib.rs` (`Palette`, `lie_cuts`, the new `draw`, two read-only hooks `occupied_screen` and `lie_screen`, two "What is real" rows), `web/shell.html` (the palette block, `PALETTE`, `frame` takes one array), `tools/verify_b8.py` (colour rule, 14 layers), `tools/verify_b9.py` (S3 checks). Backup: `web/shell.backup-20261119-023253.html`.

| Change | Why |
|---|---|
| **The palette is a block in `shell.html`, not in `draw`.** `lib.rs` names no colour; `frame` gets the nine roles as one array, read once per theme change | One place for every hex in both themes. Step 4's grep then has one block to allow |
| **The lie gap is cut out of the leg's stroke, not painted over in the background colour** | At 45°, a 6 px background stroke from the switch also covered the lying leg for about 3 px, which broke a set route there |
| **The describer box is sized at 6.6 px a character**, not measured | `measure_text` needs a `web-sys` feature the crate does not enable; the font is monospace |

Contrast against the canvas, from the WCAG formula: dark `--trk` 1.85:1, `--route` 21:1, `--occ` 5.37:1; light `--trk` 1.68:1, `--route` 21:1, `--occ` 6.54:1. **Unlit track is faint.** That is the panel convention (PN-01), and it is below the 3:1 that WCAG asks of graphics; your call after a shift.

**Found, not fixed: 13 of 75 drawn points have both legs leaving the switch on one line.** At 6 (points 5, 10, 20, 25, 38, 53) the legs lie on top of each other for 8 m to 1,088 m; points 38 and 53 are one pair of parallel edges between nodes 60 and 90. No lie can show there, and a route over either leg draws the same. Why the S1 gate `no_two_runs_share_a_row_where_they_overlap` passes over them was not traced. `verify_b9.py` lists and skips them. Fixing it is S1 layout work.

**Run 2026-11-19, after `./build.sh` (`751072 bytes`):** `cargo test --release --workspace` 76 passed, 0 failed over 13 binaries. `verify_b2.py` 37/37, `verify_b6.py` 75/75 (the 44 px check included), `verify_b7.py` 36/36, `verify_b8.py` 61/61 (colour rule: 9 palette declarations, no colour literal in `lib.rs`; frame time at Peak median 0.80 ms, 95th percentile 1.30 ms), `verify_b9.py` 44/44. S4's README commands are the ones above, run as written.

**Mutant check:** with `LIE_GAP_PX` set to 0, the four lie checks failed (2 of 16 and 6 of 15 points read as gaps, from anti-aliasing), 40/44.

## Phase 0 for this change

| Question | Answer |
|---|---|
| The one thing it must never get wrong | **A drawn route that differs from the route the sim set.** Guard: `every_route_draws_unbroken` and `edges_meet_at_their_nodes` in S1, and the pixel walk in `verify_b9.py`. A wrong click target is second: the 44 px check stays |
| Honest confidence | Track order and connections: from the yard, as solid as the yard. Lengths and angles: drawn, and the panel says so. Colours: `assumed` until S0 finds a source |
| When the data dies | Nothing new is fetched at run time. The layout is computed from the baked yard on load; if it panics, the page shows the existing wasm-failed banner |

## Design

- **Look.** The header, the rack and the buttons keep Nothing OS Classic (`md/designs.md § Nothing OS, two styles › Classic`). The canvas follows the Indian Railways panel convention in `research/panel.md`. **No catalogue entry covers a control-room mimic:** this is your instruction in this session, not a handbook pick.
- **Motion.** Unchanged: 100 ms linear presses; aspects and track colours change on the tick, as relay indications do; pan and zoom keep the `MOTION.gesture` spring.
- **Interaction.** UI-tips No. 12, hit slop: unchanged rule, re-measured.

## Out of scope

- **A hand-drawn schematic file.** If S1 cannot reach 0 false crossings, the fallback is an override file in the `[drop]` / `[kind]` style. It is not built unless you choose it at the S1 stop.
- **Laying out the 144 yard edges.** They need 65 rows and form an ordering cycle. They stay stubs; no train runs on them.
- **A train describer with berths, route indicators, calling-on, and track-circuit failures.** The describer is a number label only.
- **Any change to rules, routes, scoring or timing.** The sim crate loses one constant and nothing else.
- **Sound, phone layout, publishing, commits.** Each waits for your yes.
