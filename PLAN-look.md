# cabin control-room look — plan

Written 2026-11-19, against `games/cabin/` at commit `b2ff0d5` (the schematic panel, S0 to S4).
Method: your photo of the TTC Transit Control Centre wall (Toronto), its colours sampled with PIL over the wall area (2410 × 680 px, pixels brighter than 90). Three web searches: the TTC display's colour key, Indian Railways' VDU and panel colours, and control-centre overview walls. `research/panel.md` and the Signal Engineering Manual rows re-read. `web/shell.html` header and `crates/web/src/lib.rs` `draw` re-read.
**Built: L1 to L5, 2026-11-19.** Corrected 2026-11-19: until then this line said nothing below was implemented. "L1 to L5 built" at the end records the changes and the runs.

**Decided 2026-11-19, in chat:** question 1, TTC colours; question 2, dark only.

## Goal

**The panel and its header look like the photo: a black wall of lit pale-blue track, bright route and occupancy colours, white train boxes, an LED clock and a dot-matrix board.** The sim, the schematic layout and every control stay as they are.

## What the photo shows

| Element | In the photo | Sampled (camera-washed) |
|---|---|---|
| Background | Pure black wall | — |
| Track, clear | Lit, pale blue, thin, every line visible | `#a8c0f0`, 254k px: most of the ink |
| Long runs | Bright green | `#90c078`, 56k px |
| Short sections | Red | `#783030`, 1.7k px |
| A few runs | Magenta / purple | `#301860`, 6.8k px |
| Trains | Small white boxes on the track, dark text | `#c0c0f0`, 5k px |
| Stations | White bold sans names, small green or red time text under each (`01:23`) | — |
| Crossovers | Drawn as an X between two parallel tracks | — |
| Header | Green seven-segment clock `06:44:38`, small seconds. A red and amber dot-matrix board: `DISPATCH AGENTS:0` / `CALLS WTG:0 AVAIL:0` | — |

**What the colours mean on the TTC wall is not sourced.** The searches found no colour key for the Transit Control Centre display. Green reads as route set, and red as occupied, from how they lie on the track; that is a reading of the photo, not a fact. The magenta runs have no reading.

## Part A — findings, ranked

### A1 · INCONSISTENCY (high): the photo's colours contradict the sourced Indian Railways ones

S3 drew the Indian Railways convention, cited as `primary` in "What is real": clear track unlit (PN-01), a set route white (PN-02, PN-08; the Signal Engineering Manual says "a row of white lights"), occupied red (PN-03). The photo lights clear track pale blue and shows routes green. Red for occupied agrees. Green in the IR notes is a point-normal lamp, not a route.

**Fix, your choice (question 1):**
- **TTC look, TTC colours.** Track pale blue, route green, occupied red. The "Indications" row changes from `primary` to `authored`: "the look of the TTC control centre, Toronto; an Indian Railways panel shows the route in white." Smallest change that matches the photo.
- **TTC look, IR colours.** Black wall, lit pale-blue track, white route, red occupied. White route on pale-blue track separates by brightness only: about 1.5:1 between the two, measured before build.

### A2 · DESIGN RISK (medium): the header is Nothing OS, the photo is LED hardware

The header now has white-on-black mono labels and buttons with hard 5 px shadows. The photo's header is a seven-segment clock and a dot-matrix board. No font for either is on this machine, and a web font breaks the offline rule unless embedded.
**Fix:** draw both. The clock: inline SVG, seven segments per digit, lit in green, unlit segments at 8% so the digit shape shows, as real LED clocks do. The board: a 5 × 7 dot font as a table of 40 glyphs (A–Z, 0–9, `:`, `.`, `/`, space), drawn on a small canvas in red and amber. About 60 lines. The rack buttons keep their 3 px border and hard shadow (your 2026-11-15 rule), on the black.

### A3 · DESIGN RISK (medium): a control-room wall has no light version

`memory.md` says both themes, always, unless a look is light-only by your choice. This look is dark by nature.
**Fix, your choice (question 2):** dark only (the T key and theme button go, `cabin.v1` keeps the field and ignores it), or keep light with the same roles in darker hues.

### A4 · COSMETIC (low): platforms carry no dwell time

The photo prints a small time under each station. The sim already counts each train's dwell (`World::describe`, the rack's "at platform" bay). **Fix:** under each platform label, the berthed train's dwell left, `mm:ss`, in green; red once it is due out and has no route. Reads existing data; no sim change.

### A5 · NOISE (low): magenta is not used

No meaning was found for it. Not drawn.

## Part B — the build

| # | Slice | Done when |
|---|---|---|
| L1 | **Palette.** New values in the `web/shell.html` palette block per question 1 and 2; `--trk` 2 px at full brightness; train describers become filled white boxes with black text; platform and portal labels white bold sans; signal names stay dim mono | `verify_b9.py` passes (it reads colours by role); `verify_b8.py` colour rule passes with the new hexes; contrast of every role against black quoted |
| L2 | **LED clock.** SVG seven-segment `HH:MM` plus small `SS`, green, in the header's centre; the old text clock goes | `verify_b7.py` clock checks read the new element; same time as `app.clock_s()` |
| L3 | **Dot-matrix board.** Mode, seed and the three scores as `HANDLED 3  DELAY/TRAIN 4.2  PUT-BACKS 0` in amber, the status line's refusals in red | `verify_b7.py` score checks read the board's text (kept in an `aria-label`, so screen readers and the scripts read it as text) |
| L4 | **Dwell under platforms** (A4) | `verify_b9.py`: a berthed train's dwell shows under its platform and counts down |
| L5 | **Docs.** README, `research/panel.md` (a row for the photo, tagged `photo, unsourced key`), "What is real", `tools/README.md` | Screenshots at 1920 and 1500 px shown to you |

Backup of `web/shell.html` before L1. `./build.sh`, then all five verify scripts and `cargo test --release --workspace` after L5.

## Design

- **Look.** Your photo of the TTC Transit Control Centre. No catalogue entry covers a control-room wall; this is your instruction, as in `PLAN-schematic.md`.
- **Motion.** Unchanged: indications change on the tick, as relays and LEDs do. No glow, no flicker, no scanlines.
- **Interaction.** Unchanged. Hit slop (UI card No. 12) re-measured: `verify_b6.py`'s 44 px check.

## Out of scope

- **The photo's multi-row wall layout.** It stacks four lines across one wall. NDLS is one station; the schematic stays one block.
- **The 6 points whose legs overlap** (`PLAN-schematic.md`, found 2026-11-19). Separate S1 work.
- **Deploying the new look to Pages.** Needs your yes after you see the screenshots.

## L1 to L5 built 2026-11-19

**Built as written, with four changes.** Backup: `web/shell.backup-20261119-033905.html`.

| Change | Why |
|---|---|
| **The readout stays as text, visually hidden (`.sr`), and the clock and board draw from it** | Screen readers and every verify script read `#clock`, `#delay` and the rest unchanged; the board cannot disagree with the text it is drawn from |
| **A migrated store is written back on load, and Guard 3 writes a readable store as soon as the bad copy is kept** | The theme button was the only save before a shift's end; the scripts used it to force a write. Writing on load is the usual migration rule and needs no click |
| **The board shows bests in brackets: `HANDLED 1(1) DELAY 4.2(3.1) PB 0(1)`** | 34 characters fit at 2.5 px dots beside the clock at 1500 px |
| **Platform names are white bold sans at 12 px; signal names stay dim mono at 10 px** | The photo's station names are the brightest text; signal names would compete with them |

Contrast against black: `--trk` 11.84:1, `--route` 11.48:1, `--occ` 5.37:1, `--desc` 21:1, `--led-g` 15.69:1, `--led-a` 11.46:1, `--led-r` 5.92:1. **Route against track is 1.03:1:** they differ by hue (green against pale blue) and width (3 px against 2 px), not brightness, as on the photo. Occupied against track is 2.21:1.
