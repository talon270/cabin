# cabin redesign: a panel that lines up, a way in, a ladder of stations — plan

Written 2026-11-19, against `games/cabin/` at commit `2d555b8` (the TTC look).
Method: your report ("the lines don't line up, it's overwhelming, I don't understand how to start"). Screenshots of the default and the zoomed-out view at 1920 × 1080. A read-only probe of `Schematic::new` on the baked yard (added to `schematic.rs`, run, removed). Playwright counts of what the default view shows. Two Codex jobs (`codex --search exec -s read-only`): control-room conventions (`research/control-rooms.md`, 35 rows) and a small-station ladder (`research/stations.md`). Claude has not opened Codex's URLs. Overpass queries from this machine: 3 endpoints, all timed out.
**Nothing below is implemented — this is the plan.**

**Decided 2026-11-19, in chat:** question 1, the Indian Railways VDU look (dark, LED clock kept). B0: you run the fetch on another network; B1 starts meanwhile.

**Progress, 2026-11-19.** B0: `fetch-osm.sh <code> <box>` added; the NDLS query it sends is byte-identical to the old script's (checked with a stub `curl`); the fetches are yours. B1, part built: every vertex on a 30 m grid (gate `every_vertex_is_on_the_grid`), 49 columns for 140 nodes (was 86), yard stubs removed (was 17), 6 line ends marked (4 buffer stops, 2 dashed into yard; 0 unmarked), station area at half length so rows draw 22 px apart (was 18) with 0 head pairs under 44 px. Not built from B1: platform boxes and heads with no stalks (9 heads still slide back). B5 built: the IR VDU palette, `--trk #6a6a70` (3.91:1), route white. Run after: `cargo test --release --workspace` 77 passed, 0 failed; `verify_b2` 35/35, `verify_b6` 74/74, `verify_b7` 37/37, `verify_b8` 61/61, `verify_b9` 42/42. B2 to B4 wait for B0.

**B0, 2026-11-19, later.** HUK and NUR fetched (yours) and extracted: both pass the gate after an extractor fix for platforms mapped as open lines (`research/stations.md` § Measured). `cabin-osm <code>` reads `data/<code>.box`, which `fetch-osm.sh` now writes. SBB and DEE: Overpass HTTP 502, retry. Found while doing it: **B3 needs a design step.** The traffic model is NDLS's 1,776-line researched profile; a ladder level needs a short authored timetable (a few trains, their portal, time and dwell), and whether real train numbers that pass HUK and NUR can be sourced is open.

## Goal

**A new player opens cabin, is told what to do first, works a 2-platform station, and climbs to NDLS one skill at a time, on a panel whose lines sit on a grid like a real signalling VDU.**

## Part A — audit, ranked

### A1 · BUG (high): the first thing to do is off the screen

On a fresh Practice shift, seed 1: the first train is offered at portal N4; the default view shows 32 of 44 signal heads and **0 of the 12 Homes**; the status line says "Click a signal to choose the entry of a route". Clicking any visible signal cannot help that train: it waits at a Home off the view, and the only way to it is Accept, then the rack's Route button. Nothing on the page says so.
**Fix:** a guided first level (B4) and, on every level, a "next step" line that names the control to press and outlines it.

### A2 · DESIGN RISK (high): NDLS is the first level

16 platforms, 12 portals, 44 signals, 252 routes, all at once. The 32 heads on the view are the same red dot with equal weight. Real training starts with layout and signals, then normal procedures, then changing situations, then failures (`research/control-rooms.md` TRN-01, TRN-03, TRN-05; DMRC and Network Rail, primary, not opened by Claude).
**Fix:** a ladder of real stations, smallest first (B3, B4). NDLS becomes the last level.

### A3 · BUG (medium): the lines do not line up

Measured on the baked yard:

| Measure | Value | What you see |
|---|---|---|
| Distinct x positions of 140 drawn nodes | 86 | Jogs start and end at many slightly different places |
| Neighbouring x positions under 15 m apart | 35 pairs | About 5 px at the default zoom: two lines that almost align and don't |
| Track ends that stop in mid-air (drawn degree 1, not a portal) | 6 | Diagonals ending a few px short of a track |
| Yard stubs | 17 | Short thin lines going nowhere |
| Head stalks | 9 heads on the default view | Thin lines from a head to its post, read as extra track |
| Platform label offset | 9 px above a 20 px row | A label sits halfway between two tracks |

The schematic places x by true distance along the yard (`ALONG`, `ALONG_STATION`), so no two features share a column unless they are within 6 m. A VDU diagram is drawn on a grid (`research/control-rooms.md` DR-03: "predominantly horizontal track runs"; no source fixes a spacing, DR-05).
**Fix (B1):** snap every drawn node to a column grid whose step equals the row step, so a 45° jog over k rows spans exactly k columns and every line end meets another on a grid point. Drop the 17 stubs. Join or drop the 6 loose ends. Platform names go inside a platform box drawn beside its track, not between rows. Heads stand on a grid point beside their track; no stalks.

### A4 · INCONSISTENCY (high): "copy control rooms exactly" contradicts the TTC look

The only primary sources for an Indian Railways screen say: clear track unlit, a set and locked route white, occupied track red over any route (IN-01, IN-02; SEM §7.172 and the 2023 track-detection chapter, the same rows S3 used). A CAMTECH handbook shows a WestRACE operator VDU in Indian service: dark background, horizontal schematic tracks, line labels, drop-down menus for points, routes, signals and cancellation (IN-08, primary, not opened by Claude). A UK Controlguide report also gives white route and red occupied (UK-05, secondary). **No source gives the TTC wall's colours a meaning** (TR-05). No source fixes exact geometry, spacing or label rules (DR-05).
**Fix, your choice (question 1):** draw the Indian Railways VDU, the convention the sources support, and keep the dark-only, LED-clock header; or keep the TTC colours.

### A5 · MODEL GAP (high): the station ladder is unmeasured

Codex proposes Holambi Kalan (HUK, 2 platforms), Narela (NUR, 2 platforms, 5 tracks), Sahibabad Junction (SBB, 5 platforms), Delhi Sarai Rohilla (DEE, 5 platforms, terminal), then NDLS (`research/stations.md`). Platform counts come from station-listing sites; switch counts are estimates. Overpass did not answer from Codex's sandbox (DNS) or from this machine (overpass-api.de, overpass.kumi.systems and overpass.private.coffee each timed out). **Whether OpenStreetMap maps these stations well enough for `cabin-osm` is not known.**
**Fix:** B0 measures them first, from a network that reaches Overpass. If a station fails the gate, the next candidate replaces it.

### A6 · DESIGN RISK (medium): one station is baked in

`NDLS` and `NDLS_TRAFFIC` are `include_str!` constants (`crates/sim/src/lib.rs:473`, `traffic.rs:24`); `fetch-osm.sh` writes `data/ndls.osm.json` only. Small stations have no researched traffic.
**Fix:** a station id through `fetch-osm.sh`, `cabin-osm`, the sim and the page (B2). Ladder levels use an authored timetable of a few named trains, tagged `authored`; NDLS keeps its researched one.

## Part B — the build

| # | Slice | Done when |
|---|---|---|
| B0 | **Measure the ladder.** `./fetch-osm.sh <code> <bbox>` for HUK, NUR, SBB, DEE, run on a network that reaches Overpass (this one does not). `cabin-osm <code>` on each; quote its report: platforms, points, portals, gate | Each station passes the gate or is replaced; counts quoted in `research/stations.md` as measured |
| B1 | **Grid layout.** Column snap, no stubs, no loose ends, platform boxes, heads on grid points. New `schematic.rs` gates: every node x is a multiple of the column step; no drawn node of degree 1 except portals and buffer stops; label boxes do not overlap | `cargo test -p cabin-web` prints the A3 table again with 0 in the last four rows; `verify_b6.py` 44 px check passes; screenshots shown |
| B2 | **Stations as data.** `data/<code>.yard`, `.traffic`; `World` and `App` take a station; `?station=` | `cargo test --workspace` passes on NDLS and every ladder station; NDLS unchanged byte for byte |
| B3 | **The ladder and progress.** A level per station with one goal ("work 3 trains with no put-back"); the next level unlocks on the goal; progress in `cabin.v1`, schema 3 with its migration | `verify_b10.py`: a fresh profile starts at level 1; a goal met unlocks level 2 and survives reload; a schema 2 store migrates |
| B4 | **The guided first level.** On HUK: a next-step line ("Accept 12345 at the Down Home: press Accept"), the control outlined, one step at a time: accept, set the route in, wait the dwell, set the route out. Skippable; never shown again once done | `verify_b10.py` plays level 1 by following the line's instructions alone, with real clicks |
| B5 | **The look**, per question 1 | `verify_b9.py` colour checks for the chosen palette; `verify_b8.py` colour rule |
| B6 | **Docs.** README, "What is real" rows for each station, `tools/README.md`, `STATE-projects.md` | Commands run as written, output quoted |

**Order:** B0 first: it is the least certain part, and the ladder depends on it. B1 does not depend on B0 and can start at once on NDLS.

## Out of scope

- **A pixel copy of any vendor's screen.** No public source fixes one (IN-11, UK-06, UK-07). The panel copies the documented conventions only.
- **Failures** (track-circuit failure, point failure). Training reaches them last (TRN-05); a later plan.
- **The 6 points whose legs overlap** (`PLAN-schematic.md`). B1's grid may fix them; if not, they stay listed.
- **Traffic research for the ladder stations.** Their timetables are authored and say so.

## B2 to B4 built 2026-11-19

**Built, with these changes from the table above.**

| Change | Why |
|---|---|
| **Sahibabad dropped** (you, 2026-11-19); the ladder is HUK, NUR, DEE, then NDLS | Codex's box held no platform and nothing named Sahibabad |
| **Trains invented, numbered `L101` and up** (you chose) | Indian Railways never numbers a train with a letter, so none can be mistaken for a real one |
| **A level is done when every scripted train has left by a portal**, not "3 trains with no put-back" | A beginner's put-back is a lesson, not a failure; put-backs still show on the board |
| **NDLS is never locked** | Its shifts were the whole game until today; the ladder suggests an order and does not take NDLS away |
| **The guide reads the rack** and names one control, outlined, or one signal, ringed on the panel | One source of truth for what each train needs |
| **`[ref]` override** names a platform OpenStreetMap leaves unnumbered; **`[drop]`** removes DEE's stray platform | HUK and NUR give no platform a `ref` |

Two extractor bugs found and fixed on the way: platforms mapped as open lines (HUK, NUR) never attached, and a `ref` written `2, 3` (DEE) broke the yard file. NDLS's extract stayed byte-identical (sha256) through both.

Run after: `cargo test --release --workspace` 80 passed, 0 failed (new: `b10_levels.rs`, 3 tests; a plain controller finishes HUK in 20:11, NUR in 35:47, DEE in 34:47 game time with the invariant checker on every tick). `verify_b2` 35/35, `verify_b6` 74/74, `verify_b7` 37/37, `verify_b8` 61/61, `verify_b9` 42/42, `verify_b10` 18/18.

**Still open:** platform boxes and heads with no stalks (B1); a tighter DEE box; the guide is shown on every level and cannot be hidden.

**B1 and B4 finished, 2026-11-19, later.** Platform boxes: each platform's name and dwell sit in a box beside its track, on the describer's side, cut to the longest stretch no head or signal name stands in; before that cut 8 of NDLS's 16 boxes and 1 of DEE's 5 held a head, a name or a track line, after it 0 (`verify_b10.py`). Stalks removed: a head that slides back is drawn with no line to its post, because a white line read as a set route. Heads stay where `place_heads` puts them, not on grid points: no grid point beside a track is 44 px from every other head at the default view, so the plan's "heads on grid points" was not built. The guide has a Hide button and a command-menu toggle, kept in `settings.guide` (no schema bump: a store without the key shows the guide). Not built as planned: "never shown again once done"; levels 2 and 3 teach new skills, so the guide stays until hidden. DEE's box: three tighter boxes round the platforms each failed the gate (3 of 5 platforms attached, each reached from 2 portals only); `data/dee.box` stays as fetched. `verify_b9.py` now pauses before its countdown check: unpaused, real time ran between the two reads and 1 run in 2 failed.

Run after: `cargo test --release --workspace` 80 passed, 0 failed. `verify_b2` 35/35, `verify_b6` 74/74, `verify_b7` 37/37, `verify_b8` 61/61, `verify_b9` 42/42 (3 runs), `verify_b10` 26/26.

**Still open:** a tighter DEE box, which needs its throats read off a map; the 6 points whose legs overlap.
