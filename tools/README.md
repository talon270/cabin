# tools

Playwright scripts that drive `dist/index.html` with real clicks, drags and
wheel turns. Run `./build.sh` first: a script run on an old page proves
nothing about the change (`GUIDE-games.md` F3). Give each a scratch folder for
its screenshots; each exits 0 only if every check passed.

| Script | Covers | Last run |
|---|---|---|
| `verify_b2.py` | Skeleton: fresh, seeded, corrupt and future-version profiles; both themes at 1920 px; no network request; yard drawn; pan 1:1, rubber band and spring back, zoom about the cursor, reduced-motion zoom; the sim clock runs at `TIME_SCALE` | 2026-11-19, after S3: 37/37 passed. 2026-11-18, after per-train delay: 37/37, the schema 1 to 2 migration added. After B8: 36/36; B8 changed its corrupt-store check to Guard 3. It checked the skeleton train until B6 removed it (37/37 on 2026-11-16) |
| `verify_b6.py` | Controls: a route set and run on seeds 1 to 3 by real clicks; each refusal's text in the status line; route preview; Esc, Delete, Space, T; the command menu (open, one Esc, focus back, `hn1 p4` + Enter); the 44 px check at the default view; `?seed=` replay and a malformed seed | 2026-11-19, after S3: 75/75 passed. 2026-11-18, after the head fix: 75/75 (the 44 px check failed with 17 pairs before it). OverlapOccupied is not reached on the page. B7 raised its dwell waits to 1800 s and dropped the live wait from the seed comparison |
| `verify_b7.py` | Shifts: `?mode=` and a malformed one; header mode, window, clock and the three numbers; delay at a red Home; a put-back by the Cancel route button; a handled train; the end at 08:00, the pause and the saved best; a best after reload; a per-field merge with another tab's save; an unreadable store left untouched when `cabin.v1.corrupt` is taken; Practice unscored; real train numbers and names; both themes | 2026-11-19, after S3: 36/36 passed |
| `verify_b9.py` | Schematic panel: three routes set by clicks on seed 1 draw unbroken in the route colour (canvas sampled every 1 px; an unset route must read gapped); the route colour is white on dark, black on light; each point's leg not lying is broken at the switch, the lying leg is not; an occupied edge reads `--occ` after a train is run in by clicks. Both themes, 1920 and 1500 px. Points whose two legs are drawn on one line are listed and skipped | 2026-11-19, after S3: 44/44 passed. With the lie gap set to 0: 40/44 |
| `verify_b8.py` | Harden: every row of the plan's edges table made to fail, with what the page or tool printed (Overpass on a canned answer, `cabin-osm` on a copy of `data/`, the ledger test on an edited copy of `rails/research.md`; storage corrupt, future, blocked and full; wasm refused; two tabs; a bad seed); Guard 3; the "What is real" panel in both themes; reduced motion; the colour rule; frame time at Peak | 2026-11-19, after S3: 61/61 passed; the colour rule now checks the palette block, and the panel has 14 layers. Runs `cargo`, so it takes a few minutes. It never writes `data/` or `rails/` |

```sh
./build.sh
python3 tools/verify_b2.py /tmp/cabin-shots
python3 tools/verify_b6.py /tmp/cabin-shots
python3 tools/verify_b7.py /tmp/cabin-shots
python3 tools/verify_b8.py /tmp/cabin-shots
python3 tools/verify_b9.py /tmp/cabin-shots
```
