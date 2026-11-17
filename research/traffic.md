# traffic.md — how busy NDLS is, and who runs through it

Written 2026-11-15 for `PLAN-cabin.md` slice B1.
Method: three agents read sources in parallel and each wrote one ledger in the `rails/research.md` row format (`ID | Claim | Source | Conf`). This file does not repeat their rows. It says what each ledger holds, what the game took from it, and what is weak. **I (the author of this file) did not re-open any source;** the rows are the agents' reading. Their tags are kept as written.

| Ledger | Holds | Rows | Agent |
|---|---|---|---|
| `traffic-volume.md` | Trains per day, hourly spread, direction split, type mix, platform use | V-01 to V-42 | sonnet research agent |
| `traffic-pool.md` | 139 real trains by number, name, type, times, platform; rake length; dwell; locomotives | P-001 to P-208 | sonnet research agent |
| `signalling-search.md` | One more search for real NDLS signal positions | S-01 to S-06 | haiku doc-fetcher (findings copied in by hand) |

`data/ndls.traffic.toml` is built from them. Every value there is `{ v, src, tag }`, and `cargo test -p cabin-osm` fails if a value has no tag, cites a row that no ledger holds, or has no source and is not `assumed`.

## What the game takes

| For | Value | Rests on | Tag |
|---|---|---|---|
| Trains on a busy day | 275 | V-02, V-03, V-12 | assumed |
| Hourly weights | 3 2 0 2 5 9 20 16 14 22 19 10 13 14 9 7 14 21 18 12 10 12 10 13, peak 22 at 09:00 | V-12 | secondary |
| Share from the north side | one third | V-22 to V-24 | assumed |
| Mix | 28% EMU and local, 41% express, 31% premium; Vande Bharat 0 | V-27 to V-29 | assumed |
| Pool | 139 trains, 12 types | P-001 to P-139 | secondary; 28 are `unconfirmed` |
| Rake length | 187 m (MEMU) to 528 m (Rajdhani), loco 20.6 m extra | P-156 to P-206 | mostly assumed |
| Dwell, through express | 900 to 1500 s | P-181 | secondary |
| Dwell, terminating | 600 to 1200 s | none | assumed |

## What is weak

**The only per-day source is one paper's 2016 survey.** Anand and Gupta (2018) took the figures from NDLS's berthing chart, and nobody here can see the chart. No official figure after 2016 was found: pib.gov.in, web.archive.org, Indian Express and ScienceDirect all refused the agent (V ledger, blocked sources). Wikipedia's "400 unique trains" cites the same paper, which says 351 for the whole timetable (V-01).

**The two sources disagree on the peak.** The paper's hourly chart peaks at 09:00 with 22 trains; the 2016 datameet scrape peaks at 13:00 with 26 movements and counts days of run as one (V-19). The game uses the paper's, because its 24 labels sum to exactly 275.

**The pool and the volume describe different years.** The volume is 2016; the pool comes from RailYatri pages recorded between 2017 and 2026. Of 121 train numbers in the 2016 datameet scrape, 14 no longer stop at NDLS, and only 68% of the other 107 keep the same route (traffic-pool.md, section B). The pool is a draw set, not a day's schedule.

**The pool's through-train share does not match the volume.** The pool holds 62 starting, 58 terminating and 19 through trains (14% through); the 2016 volume has 103 of 275 through (37%). The game's `Schedule` must take the originating, terminating and through split from the volume ledger, not from the pool.

**Rake lengths are mostly assumed.** Coach length is solid (24.0 m on LHB, matching the RDSO maximum, P-156); coach counts per type are secondary, and the two generator cars on a Rajdhani come from one other train's notice (P-161). EMU car length (22 m) has no source (P-179).

**Not found:** current trains per day, any current hourly or direction figure, the Vande Bharat share, freight and light-engine movements, platform allotment (P-204), terminating-train platform dwell (P-199), and real NDLS signal positions (S-06).

**Next step the volume agent proposed:** ask NDLS station management, Northern Railway or an NTES export for the current berthing chart. It would replace V-01 to V-03, V-12 and V-39 to V-42 in one step. Until then `data/ndls.traffic.toml` stands, and the game prints its traffic as "about".
