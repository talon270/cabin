# cabin

A signalling game set at New Delhi station (NDLS), built in Rust and run as
WebAssembly in one offline HTML file. The yard is OpenStreetMap's track
geometry (extract of 2026-11-15), drawn as a control-room schematic: 12 portals, 16 platform tracks, 44 signals
and 242 routes. The traffic is drawn from the seed at the rates of a 2016
survey of the station's berthing chart (about 275 trains on a busy day, 22 in
the 09:00 hour), with 139 real train numbers and names. You accept trains
offered at the portals, set routes in to a platform and out again, and the
interlocking refuses every route that is not safe. The score is three numbers,
never one.

## Run it

Open `dist/index.html` in Chrome (the verify scripts run Chromium; other
browsers are untested). It needs no server, no install and no network: the
wasm is inlined, and the page makes no request of any kind.

Rebuild and check after a change, from `games/cabin/`:

```sh
./build.sh                                                            # dist/index.html
env CARGO_TARGET_DIR=$HOME/.cache/cabin-target cargo test --release --workspace   # 76 tests, about 3 min
python3 tools/verify_b8.py /tmp/cabin-shots                           # Playwright, real clicks
env CARGO_TARGET_DIR=$HOME/.cache/cabin-target cargo run -p cabin-osm  # re-extract after editing data/ndls.overrides.toml
./fetch-osm.sh                                                        # refresh data/ndls.osm.json from Overpass; rarely
```

**Keep `CARGO_TARGET_DIR` on every `cargo` command.** Without it, cargo
writes a `target/` folder into `SyncedWork` (163 MB after one debug run of the
extractor) and syncs it. `build.sh` sets it itself.

`build.sh` needs the `wasm32-unknown-unknown` target, `wasm-bindgen-cli` at
the version in `Cargo.lock` (0.2.130), and `wasm-opt` (binaryen). It checks
the wasm-bindgen version before it builds. `tools/README.md` lists the five
verify scripts and what each covers. Run `./build.sh` before any of them: a
script run on an old page proves nothing about the change.

## Controls

| Input | Does |
|---|---|
| Click a signal | choose it as the entry; its exits are ringed and listed as buttons under the panel |
| Hover an exit | draw the route dashed |
| Click an exit, or its button | set the route |
| Accept, in the rack's "due" bay | let an offered train in (Line Clear to the station in rear) |
| Route, in the rack | choose the signal a waiting train stands at |
| `Esc` | clear the choice |
| `Delete` | cancel the chosen signal's route |
| `Space` | pause; after a shift ends, play on unscored |
| `T` | switch theme |
| `Ctrl K` / `Cmd K` | command menu: every action, a replay, a new shift. `hn3 p5` finds "Set HN3 to P5 (ST5S)" |
| Drag, wheel | pan, zoom (zoom springs; jumps under reduced motion) |
| What is real | the sources, tags and dates of each layer, and every constant with its tag |

| Mode | Traffic | Scored |
|---|---|---|
| Practice | 2 to 6 trains in 30 game minutes | no |
| Light / Normal / Peak | 0.5 / 0.75 / 1.0 of the busy day's hourly rate: about 13 / 19 / 26 trains | yes |

A shift is 90 game minutes, 06:30 to 08:00, which is 22.5 real minutes at
`TIME_SCALE` 4. At 08:00 the page pauses and saves any best. `?mode=peak&seed=7`
replays a shift: same seed, same trains, same times. The address bar always
carries the shift you are playing.

**The score is "held at red".** Trains handled counts trains that left by a
portal. Delay counts game minutes a train stood outside a portal after it was
due, at a red Home, or at a red Starter after its dwell; the dwell itself is
not delay. The header shows delay per train: the total divided by the trains
due so far in the shift, so a Light and a Peak shift read on one scale (an
idle shift on seed 1: 50.8 min per train over 14 in Light, 48.1 over 25 in
Peak). Put-backs counts routes cancelled after a train took them or while
one ran up to the entry signal. The interlocking prevents collisions, so the
score has no incidents.

## The things most signalling games get wrong

**One function decides every clearance, and a second piece of code that knows
nothing about it checks the result.** `Interlocking::request` is the only
writer of the lock table. The invariant checker in
`crates/sim/tests/common/invariant.rs`
works from train positions alone: no two bodies share a section, point or
crossing, no train stands foul of another's leg, every moving train holds a
lock ahead of it, no locked point moves. The soak ran 10 game hours on 3 seeds
(about 75,000 ticks each) and the fuzz 1,000,000 random actions (1,920,653
ticks): 0 invariant failures. The soak found two holes before that, a train
still entering at a portal that a departure could be set into head-on, and a
train offered up to a Home that fouled a junction in use. Each now has a named
test that failed before its fix.

**Every route was refused for a reason, and the reason is on the panel.** A
refusal is one line under the panel that names what is in the way: "ST6N to
N5: refused, ST6N to N4 holds track section 258." A refused request changes
nothing; the fuzz compared 10,905 refusals against a copy of the world, and
none changed it.

**The signals are generated, not real.** No public NDLS signalling plan, yard
diagram or station working rules turned up in two rounds of search (plan A3,
`research/signalling-search.md`). OpenStreetMap holds 2 signals for the whole
station. So a Home stands 30 m outside the first connection on each approach,
and a Starter 10 m before the first point at each platform end, by the
placement rules of the General Rules. `data/ndls.overrides.toml` can move or
rename a signal when a real position turns up.

**You accept each train, because the station gridlocks otherwise.** Every
portal is worked in both directions here; the real approaches are double line.
With trains offered whenever a portal was free, 2 of 3 soak seeds locked solid
at about 7.6 game hours: 10 trains berthed, 12 waiting at all 12 Homes, every
departure needing a portal track a waiting arrival stood on. Accepting a train
is giving Line Clear to the station in rear, which a signalman gives only for a
train the station can receive. A shift accepted carelessly can still gridlock, and the
delay shows it.

**A bad map extract never replaces a good yard.** `cabin-osm` compares the new
yard with the one on disk and refuses a loss of any platform, portal,
portal-to-platform path or track way. A deleted crossover, all 19 crossovers
deleted, and 4 deleted platform outlines are refused with 13, 92 and 66 paths
lost; the file is not touched. `-- --accept-worse` writes anyway, for a loss
you meant.

**The rules cite their source, and the build fails when the source moves.**
Each rule constant in `consts.rs` names a row of `rails/research.md` and the
phrase it rests on. `tests/ledger.rs` reads that file and fails if the row no
longer holds the phrase, or if the file is missing. It does not skip.

**A crowded signal head steps back, and a stalk points to its post.** Two
Starters 10 m before one junction stood 3.3 px apart at the default view, and
17 pairs were closer than the 44 px a click target needs. No stretch of the
panel separates two posts that close. So when the default view is framed,
each head takes the place nearest its post, along its own track, that is
45 px from every other head, and a thin line joins it to the post. The sim
and the trains still stop at the post. 17 pairs became 0.

**The panel is a schematic, not a map, and it says so.** The first panel
drew OpenStreetMap's polylines with the cross-track spacing stretched six
times, so every 1 m survey wobble showed as a bend. Now each through-line
sits on its own row and changes rows only by a 45° jog, as a control-room
screen draws it: 67 lines on 33 rows, 0 false crossings
(`crates/web/src/schematic.rs`, whose tests hold those gates). Track order
and connections are the yard's; lengths and angles are drawn. The station
area is at true length, the throats at 0.35, and the whole drawing is 1.88
times the true 2,256 m, because the jogs need room.

**The indications are Indian Railways', from the manual.** Clear track is
unlit, a set route is white (path solid, overlap dashed), and occupied track
is red over any route, as the track-circuit chapter (2023, §17.2.6) and the
Signal Engineering Manual (§7.172.2) say (`research/panel.md`). A train is
its lit sections and its number in a describer box, not a block. A point's
leg that is not lying is broken for 6 px at the switch; that look is
assumed, because no source says how a panel draws it.

## What is assumed and what is solid

20 constants are tagged in `crates/sim/src/consts.rs`; 19 are `assumed`. The
"What is real" dialog reads the tags from the build, so it cannot drift from
the code.

| Solid (sourced) | Assumed | What would replace the assumption |
|---|---|---|
| Track geometry, points, platforms (OpenStreetMap, ODbL, extract of 2026-11-15) | 29 yard and siding ways promoted to running track so every platform is reachable; 4 more after the N4 fix. Platform numbers assigned by side, west to east | an NDLS track diagram |
| The General Rules: a signal clears only for a set, locked, clear route (C3-139); 120 m overlap for multiple-aspect signals (C3-143) | that NDLS has multiple-aspect signals | the station working rules |
| — | every signal position (generated by rule, 30 m and 10 m setbacks) | a signalling plan, through the overrides file |
| — | 15 km/h over reversed points and 50 km/h otherwise: Southern Railway's figures (C3-49, C3-52), because Northern Railway's Subsidiary Rules were not read | NR's Subsidiary Rules |
| Hourly spread, peak 22 at 09:00 (one 2016 survey, secondary) | 275 trains on a busy day; one third from the north; the type mix | a timetable newer than 2016 |
| 139 train numbers and names (secondary; 28 unconfirmed) | rake lengths, mostly; terminating dwell 600 to 1200 s | — |
| — | 30 m fouling length, 180 s block headway, 120 s approach release, 0.3 and 0.5 m/s² | measured fouling marks; NR practice |
| Panel colours: clear track unlit, set route white, occupied red over any route (`research/panel.md` PN-01 to PN-03, PN-08) | how a point's lie and a signal at danger look on the panel (PN-04 to PN-06); the light theme's darker hues | a photo or drawing of an NDLS panel |
| Track order and connections on the panel (from the yard) | every drawn length and angle: rows 30 m apart, 45° jogs, throats at 0.35 of true length | — |
| — | `TIME_SCALE` 4, chosen from the soak: 2.0 route decisions per real minute on average, 2.6 in the peak hour | your play |

## What is not right yet

- **Three Homes off the default view stand 28 to 35 px apart.** HN3, HN4
  and HN4b sit on portal tracks too short for a head to step back on. Every
  head on the default view is at least 44 px from the next, at 1920 px and
  at 1500 px.
- **Delay per train still punishes an ignored offer hardest.** Each train
  waits from its due time, so one left outside all shift adds up to 90 min
  to the total. Per train, it no longer swamps the rest of the score.
- **At 13 of 75 drawn points, both legs leave the switch on one line.** At
  6 of them (points 5, 10, 20, 25, 38 and 53) the two legs lie on top of
  each other for 8 m to 1,088 m, so the lie gap cannot show and a route over
  either leg looks the same. The schematic layout puts them there; the
  sim's routes are right. `verify_b9.py` lists them and skips them.
- **Unlit track is faint by design.** Dark track `#3a3a3c` is 1.85:1 against
  black; light `#c7c7cc` is 1.68:1 against white. A real panel is dim when
  clear; whether it is too dim to play is your call.
- **The 144 yard edges are stubs.** They need 65 rows and form an ordering
  cycle; no train runs on them.
- **Platforms 15 and 16 are not worked.** Their only arrivals came from the
  south into berths with no departure, so those routes were removed.
- **29 of 32 Starters stand inside the 30 m fouling length** of the junction
  ahead. A berthed train blocks every route over that junction. Safe, and slow.
- **66 of 192 portal-platform pairs have no path.** Whether that is the map or
  the real station is not known.

## Data, licence, privacy

The track data is © OpenStreetMap contributors (ODbL). The panel's corner
shows that credit with the extract date, "schematic · lengths and angles not
to scale" and "not for real-world use". The derived files in `data/`
carry the same licence. The traffic research lives in `research/`, one ledger
row per fact, with its source.

The page stores your theme and your bests, per field and per mode, in
`localStorage` under `cabin.v1` (schema 2; a schema 1 delay best was a shift total, and the
migration keeps it under `v1_delay_totals` rather than show it as a per-train best). An unreadable or future-version
copy is kept under `cabin.v1.corrupt` and a banner says so. It sends nothing
anywhere. The repo is private on GitHub, `talon270/cabin`.

The build history, every slice's measurements and the open decisions are in
`PLAN-cabin.md`.
