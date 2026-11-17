# traffic-volume.md — the NDLS traffic fact ledger

Written 2026-11-15 for `PLAN-cabin.md` slice B1 part 1: how many trains New Delhi (NDLS) handles, so the generated traffic has an honest pace.
Method: read one conference paper that quotes the station's own berthing chart (`AG18`), counted the datameet 2016 timetable by script (`DM16`), and read Wikipedia only to trace its figures to their owners. Every Indian Railways, Railway Board and PIB URL I tried either returned HTTP 403 or was refused by the fetch tool, so **no number below is `primary`**: no official Northern Railway or Railway Board figure was readable. Page numbers are PDF page indexes.

**Every row is one fact, and a number with no source is not written as a fact.** `secondary` is a published paper or encyclopedia that is not the owner of the number. `derived` is counted by me from `DM16`, with the method in the claim. `unconfirmed` is a figure I saw but could not trace to its owner. `UNSOURCED` is a hole: the row says what I looked for and where.

## Sources

| ID | Document | Status |
|---|---|---|
| `AG18` | Anand, Rohit and Gupta, Sanjay, "Productivity of Railway Stations: Case Study - New Delhi Railway Station", *7th Transport Research Arena TRA 2018*, Vienna, 16-19 April 2018. DOI 10.5281/ZENODO.1491536. 10 PDF pages, `zenodo.org/records/1491536`. Authors from the School of Planning and Architecture, Delhi. Its train data comes from "the current Platform Berthing Chart from New Delhi Railway station management" (p3) plus the 2008 Terry Farrell redevelopment report; the survey year is 2016 (Table 2, "Primary Survey, 2016"). | Read 2026-11-15: pp1-5 and p9 in full by text extraction, `grep` over the rest. **Fig. 4 (hourly chart) is an image with printed bar labels**, read from the page-5 render at 130 dpi: the 24 labels are legible and sum to 275, the paper's own daily maximum (V-12). The berthing chart itself was not seen. |
| `DM16` | datameet/railways, `schedules.json` (82,174,280 bytes, 417,080 stop rows), `trains.json` (14,768,598 bytes, 5,208 trains) and `stations.json`, `github.com/datameet/railways`. No licence file. Latest commit touching `schedules.json` is dated 2016-08-08; the README points to a 2016-08 blog post on "gathering Indian Railways data". Has no days-of-run field. | Downloaded 2026-11-15, counted by `python3 -I`. Secondary and 2016: it is a scrape, not an Indian Railways publication. Scripts were kept in the scratchpad, not the project. |
| `WP-NDLS` | Wikipedia, "New Delhi railway station". | Read 2026-11-15: lead, infobox, history and the reference list (references 26-37). Its train figures cite `AG18` (ref 36) and a RISD thesis (ref 37); neither supports every figure (V-06). `secondary`. |
| `PIB-74452` | PIB, "Train Operation from Delhi Stations", 12 August 2011 (cited by `WP-NDLS` ref 26 for passenger footfall). | **Not available.** `pib.gov.in` returned HTTP 403; the fetch tool refused `web.archive.org`. |
| `IE-2022` | The Indian Express, "New Delhi Railway Station to be revamped", 4 September 2022. | **Not available.** The fetch tool refused `indianexpress.com`. |
| `BR22` | Bhatnagar and Ram, "Assessment of level of service for access and dispersal facilities in a railway terminal", *IJTST* 11(2), 2022, DOI 10.1016/j.ijtst.2021.04.010. | **Not available.** ScienceDirect returned HTTP 403. `WP-NDLS` cites it for "over 250 passenger trains every day"; unchecked. |
| `RM-NDLS` | RailMitra, NDLS station page, `railmitra.com/stations/new-delhi-ndls-railway-station`. | Opened 2026-11-15. The page I fetched contains no train counts. A search-engine summary attributed "162 originating, 144 passing, 162 terminating" to this site; I could not find it there (V-07). |
| `SEARCH` | Web-search result snippets for NDLS trains per day and Vande Bharat services from NDLS (railmitra, ixigo, travel sites). | Snippets only, pages not opened. Used for leads (V-35) and never for a number in a claim. |
| `RB-NTES` | Railway Board, Northern Railway, NTES and IRCTC timetable publications. | **Not obtained.** No official publication of NDLS traffic was reachable. `indianrailways.gov.in` also served HTTP 403 in the earlier ledger (`rails/research.md`, `IRPWM-2024`); I did not retry it. |

---

## Q1 — Trains per day

| ID | Claim | Source | Conf |
|---|---|---|---|
| V-01 | **351** trains are operated from NDLS: 106 originating, 106 terminating and 139 passing. Year: survey 2016, from the station's berthing chart. This is the whole timetable, not one day (see V-02). | `AG18` §5.2.1 p5 | secondary |
| V-02 | At most **275** trains are scheduled on any one day: 87 originating, 87 terminating, 103 passing. Same year and chart as V-01. | `AG18` §5.2.1 p5 | secondary |
| V-03 | V-02's three parts add to 277, not 275. The paper's hourly chart sums to 275 (V-12), so 275 is the figure to use and the 87/87/103 split is off by 2 somewhere. | `AG18` p5, summed by me | derived |
| V-04 | "About **245** trains operated everyday from **12** platforms", from "secondary data", no year given. The 12 platforms contradicts the present 16 (V-33), so this is an older figure, dated before the station grew to 16 platforms. | `AG18` §5.2.2 p5 | secondary |
| V-05 | "Over **350** trains daily with 16 platforms". This is loose wording for V-01's weekly union; it conflicts with V-02's 275 per day. | `AG18` §4.2 p4 | secondary |
| V-06 | Wikipedia says NDLS "accommodates a total of over **400 unique trains**", "over **250 passenger trains** every day", and 13-23 trains per platform per day. The 400 is cited to `AG18`, which says 351 (V-01), so it does not trace to its stated owner. The 250 cites `BR22` (not read) and `AG18`. | `WP-NDLS` lead and refs 30, 36, 37 | unconfirmed |
| V-07 | A search summary attributed 162 originating, 144 passing and 162 terminating trains to RailMitra. It is not on the page I fetched, and its 162 originating exceeds `AG18`'s 106 (V-01). Not used. | `SEARCH`, `RM-NDLS` | unconfirmed |
| V-08 | The 2016 datameet timetable has **233** distinct train numbers with an NDLS stop: **80** originating (NDLS is the first stop), **80** terminating (last stop), **73** through. Counted by sorting each train's stop rows by row `id` (ids run in route order, checked on 12260 and 12031) and testing whether NDLS has a previous and a next stop. Year: scraped 2016. **It is a timetable count, not a per-day count**: there is no days-of-run field, so a weekly train counts the same as a daily one. 3 of the 233 are `-Slip` coach entries, and 4 through-train rows carry no times. | `DM16` | derived |
| V-09 | V-08's 233 is 66% of V-01's 351 (233/351). `DM16` has only 4 MEMU and 5 EMU entries for NDLS, against `AG18`'s EMU share of 28% (V-30); that is the likely gap, but I did not establish it. | `DM16`, `AG18` | derived |
| V-10 | All 233 NDLS trains in `DM16` are passenger-carrying: the `type` field takes the values SF, Exp, Raj, Shtb, JShtb, Drnt, SKr, GR, Pass, MEMU and `Del` (an EMU "Parikrama" ring service, by train name), and none is a goods type. So "passenger vs other" has no answer here; see V-37 for non-passenger movements. | `DM16` `trains.json` | derived |
| V-11 | An official trains-per-day figure for NDLS in any year after 2016 (Railway Board, Northern Railway, Delhi Division, NTES, a Lok Sabha or Rajya Sabha reply, a PIB release). Looked for: PIB 74452 (HTTP 403), Indian Express 2022 (refused), ScienceDirect `BR22` (HTTP 403), two web searches for an official count (only travel sites and Wikipedia came back). | `PIB-74452`, `IE-2022`, `BR22`, `RB-NTES` | UNSOURCED |

---

## Q2 — Hourly spread

Hour `h` below means the clock hour `hh:00` to `hh:59`, with `h` from 0 to 23.

**Reading Fig. 4.** The chart's x-axis runs 1 to 24. The text puts the morning peak at "9am to 10am" with 22 trains and the evening peak at "5pm to 6pm" with 21; those are the bars labelled 10 and 18, so **bar n is hour n-1**. Bar 3 has no label and no height; I read it as 0, and the sum check (V-12) supports that.

| ID | Claim | Source | Conf |
|---|---|---|---|
| V-12 | Trains per hour on a day with 275 trains, hours 0 to 23 in order: **3, 2, 0, 2, 5, 9, 20, 16, 14, 22, 19, 10, 13, 14, 9, 7, 14, 21, 18, 12, 10, 12, 10, 13**. Sum **275**, equal to the paper's daily maximum (V-02, V-03). Each train appears to be counted once; the paper does not say whether the hour is its arrival or its departure. Year: 2016, "Daily Average". | `AG18` Fig. 4 p5, labels read from the image, summed by me | secondary |
| V-13 | Busiest hour is **09:00-09:59 with 22 trains**; next 17:00-17:59 with 21 trains, then 06:00-06:59 with 20 trains (V-12). The paper names the first two as its morning and evening peaks. | `AG18` §5.2.1 p5 | secondary |
| V-14 | Hours 00:00 to 05:59 hold **21 trains, 7.6%** of 275 (3+2+0+2+5+9); the hour 02:00-02:59 holds none. From 06:00 to 23:59 the lowest hour is 15:00-15:59 with 7. | `AG18` Fig. 4, summed by me | derived |
| V-15 | Arrivals at NDLS per hour, hours 0 to 23: **0, 2, 0, 1, 8, 7, 10, 10, 7, 9, 17, 9, 7, 16, 4, 1, 3, 5, 4, 3, 5, 10, 6, 5**. Sum 149. Method: for each NDLS stop row with an arrival time, add 1 to hour `HH` of that time; 4 of the 153 arrival rows have no times and are left out. Year: 2016 timetable, every listed train counted as running daily (see V-08). | `DM16` | derived |
| V-16 | Departures from NDLS per hour, hours 0 to 23: **5, 2, 0, 0, 6, 2, 8, 8, 4, 7, 2, 8, 2, 10, 14, 6, 11, 13, 8, 4, 8, 8, 3, 10**. Sum 149. Same method on the departure time; same 4 rows missing. | `DM16` | derived |
| V-17 | Busiest hour by movements (arrivals plus departures) is **13:00-13:59 with 26** (16 arrivals, 10 departures), then 10:00 with 19 (17+2) and 06:00, 07:00, 14:00, 17:00 and 21:00 with 18 each. A through train counts twice, once on arrival and once on departure. Total 298. | `DM16`, summed from V-15 and V-16 | derived |
| V-18 | Counting each train once, at its first NDLS event (arrival if it has one, else departure), gives per hour 0 to 23: **0, 2, 0, 1, 8, 7, 17, 17, 7, 12, 18, 11, 8, 20, 11, 5, 12, 17, 6, 5, 10, 12, 9, 14**. Sum 229 (233 minus 4 with no times); peak **20 at 13:00**. This is the series that compares with V-12. | `DM16` | derived |
| V-19 | V-12 and V-18 **disagree on the peak**: `AG18` peaks at 09:00 (22 trains) and has 14 at 13:00; `DM16` peaks at 13:00 (20) and has 12 at 09:00. Different years and different train sets (V-09: `AG18` includes 28% EMU), so neither is wrong. Use V-12 for the shape and V-18 only as a warning that the shape is uncertain. | `AG18`, `DM16` | derived |
| V-20 | Platform dwell: of the 69 `DM16` trains that both arrive and depart at NDLS, the dwell (departure minus arrival) has median **20 min**, mean 28.1, quartiles 10 and 30 min, minimum 2, maximum 150. 41 of the 69 stand 20 min or less. Originating trains show no arrival, so their dwell is unknown. Year: 2016 timetable. | `DM16` | derived |
| V-21 | A per-hour count for any year after 2016, or for arrivals and departures separately from the station's own berthing chart. | `RB-NTES` | UNSOURCED |

**Hourly table.** Every cell is copied from V-12, V-15 and V-16; none is a new fact.

| Hour | V-12 trains | V-15 arr | V-16 dep | Hour | V-12 trains | V-15 arr | V-16 dep |
|---|---|---|---|---|---|---|---|
| 0 | 3 | 0 | 5 | 12 | 13 | 7 | 2 |
| 1 | 2 | 2 | 2 | 13 | 14 | 16 | 10 |
| 2 | 0 | 0 | 0 | 14 | 9 | 4 | 14 |
| 3 | 2 | 1 | 0 | 15 | 7 | 1 | 6 |
| 4 | 5 | 8 | 6 | 16 | 14 | 3 | 11 |
| 5 | 9 | 7 | 2 | 17 | 21 | 5 | 13 |
| 6 | 20 | 10 | 8 | 18 | 18 | 4 | 8 |
| 7 | 16 | 10 | 8 | 19 | 12 | 3 | 4 |
| 8 | 14 | 7 | 4 | 20 | 10 | 5 | 8 |
| 9 | 22 | 9 | 7 | 21 | 12 | 10 | 8 |
| 10 | 19 | 17 | 2 | 22 | 10 | 6 | 3 |
| 11 | 10 | 9 | 8 | 23 | 13 | 5 | 10 |

---

## Q3 — Direction split

**Grouping rule, and why it is by neighbour.** Every one of the 306 neighbour slots around an NDLS stop in `DM16` (a previous and a next stop for each of 153 arrivals and departures) is one of two stations, so I grouped by that and not by far-end station. Far-end grouping would misplace through trains, which arrive from one side and leave by the other.

| Group | Adjacent stop after or before NDLS | What follows it, from the stop sequences | Far-end stations seen |
|---|---|---|---|
| **N** (north, "Old Delhi side") | `DSB` Delhi Sadar Bazar | Subzi Mandi, Azadpur, Adarsh Nagar (Ambala line); or Delhi Kishanganj, Sarai Rohilla, Shakurbasti (Rohtak and Jaipur direction); or Delhi Junction | ASR 12, JAT 11, SVDK 2, ROK 2, FZR 2, LPNR 2, RE 2, KLK 2, DLI 2, CDG 2, SGNR 2 and others |
| **SE** (south and east, "Nizamuddin side") | `CSB` Shivaji Bridge, then Tilak Bridge | Anand Vihar and Ghaziabad (Kanpur line); or Nizamuddin and Okhla (Mathura, Agra, Faridabad line) | LKO 6, HWH 6, DDN 4, RNC 4, BBS 4, BDTS 4, TKJ 3, MAS 3, PURI 3 and others |

| ID | Claim | Source | Conf |
|---|---|---|---|
| V-22 | Arrivals at NDLS by side: **102 from SE (66.7%) and 51 from N (33.3%)**, out of 153. Method: the previous stop in the train's sorted stop list is `CSB` or `DSB`; no other value occurs. Year: 2016 timetable, trains counted as daily (V-08). | `DM16` | derived |
| V-23 | Departures from NDLS by side: **102 to SE and 51 to N**, out of 153; the same split as V-22, because no through train arrives and leaves on the same side (V-24). Of the 51 north departures, about 26 go via Subzi Mandi and Azadpur (Ambala line), about 21 via Kishanganj and Sarai Rohilla (Rohtak, Jaipur direction) and 2 to Delhi Junction; the other 2 are on other patterns. | `DM16` | derived |
| V-24 | Patterns, as (arrival side, departure side): originating to SE 66, originating to N 14; terminating from SE 65, terminating from N 15; through SE-to-N 37, through N-to-SE 36; through trains that reverse out the way they came **0**. So 73 of 233 trains cross the station from one side to the other. | `DM16` | derived |
| V-25 | Counting per train and not per movement, 66 of the 80 originating trains leave to SE (82.5%) and 65 of the 80 terminating trains arrive from SE (81.3%): the terminals are mostly south and east, and the north side is mostly a through route. | `DM16`, from V-24 | derived |
| V-26 | The side split for any year after 2016, or by train type, from a source other than `DM16`. | `RB-NTES` | UNSOURCED |

---

## Q4 — Type mix

`DM16` gives each train a `type` code in `trains.json`. The codes are **not documented in the repository**; my reading of them is from the train names and is `derived`: `Raj` Rajdhani, `Shtb` Shatabdi, `JShtb` Jan Shatabdi, `Drnt` Duronto, `SF` Superfast (this includes Punjab Mail and Lucknow Mail, which `DM16` types SF, not `Mail`), `Exp` Express, `SKr` Sampark Kranti, `GR` Garib Rath, `Pass` passenger, `MEMU` MEMU, `Del` (5 entries, all "Parikrama" ring EMUs).

| ID | Claim | Source | Conf |
|---|---|---|---|
| V-27 | Rajdhani: **32 of 233 trains, 13.7%**. 2016 timetable. | `DM16` | derived |
| V-28 | Shatabdi and Jan Shatabdi: **25 of 233, 10.7%** (21 Shatabdi, 4 Jan Shatabdi). | `DM16` | derived |
| V-29 | Duronto: **10 of 233, 4.3%**. | `DM16` | derived |
| V-30 | Other Superfast, Mail and Express together: **143 of 233, 61.4%** (87 SF, 38 Exp, 14 Sampark Kranti, 4 Garib Rath). | `DM16` | derived |
| V-31 | Passenger, MEMU and EMU together: **23 of 233, 9.9%** (14 Pass, 4 MEMU, 5 `Del`; the 5 are 4 through plus 1 originating, all ring services). | `DM16` | derived |
| V-32 | Express trains are the largest group at **41%** and EMU trains the second at **28%**; the paper does not say the denominator, the remaining 31% is not broken out, and it names no Rajdhani, Shatabdi or Duronto share. Year: 2016, berthing chart. I read the denominator as the 351 of V-01. | `AG18` §5.2.1 p5 | secondary |
| V-33 | `DM16`'s 9.9% passenger and EMU (V-31) is far below `AG18`'s 28% EMU (V-32); the datameet scrape carries almost none of Delhi's suburban and ring EMU services (V-09). Treat 28% as the better estimate of local traffic and `DM16` as a sample of the long-distance trains only. | `AG18`, `DM16` | derived |
| V-34 | Vande Bharat in 2016: **0**. The first Vande Bharat service was flagged off in February 2019 (`PIB-188609` in `rails/research.md`, read there on 2026-10-29, not re-read for this file), and `DM16`'s type codes have none. So `DM16` says nothing about today's share. | `DM16`; `rails/research.md` | derived |
| V-35 | Vande Bharat services that depart NDLS today, seen only as search snippets from aggregators (not opened, not dated): 22439 (06:00 to Katra), 22436 (06:00 NDLS to Varanasi), 22416 (15:00 NDLS to Varanasi), 22447 (05:50, 6 days a week), 22477 (15:00 to Katra, 6 days a week), and a Patna special 02436. Treat as leads for slice B1 part 2, not facts: the count, the times and the days-of-run are unchecked, and the snippets disagree with each other on some times (one gives 22440's arrival at 23:00, another 14:00). | `SEARCH` | unconfirmed |
| V-36 | The current share of Vande Bharat and of any train type after 2016. Looked for: a count of NDLS Vande Bharat services from the Railway Board or Northern Railway; none reachable (`RB-NTES`); aggregator pages are the only place it appeared (V-35). | `RB-NTES`, `SEARCH` | UNSOURCED |
| V-37 | Freight and light-engine movements through NDLS: counts, hours or share. Looked for in `DM16` (a passenger timetable by construction, V-10: no goods type), in `AG18` (the 351 and 275 of V-01 and V-02 are not labelled passenger or goods, and the paper never mentions freight), and in `WP-NDLS` (silent). Whether the 139 "passing" trains of V-01 include any non-passenger train is not stated. | `AG18`, `DM16`, `WP-NDLS` | UNSOURCED |

---

## Q5 — Platform use

| ID | Claim | Source | Conf |
|---|---|---|---|
| V-38 | NDLS has **16 platforms** (present day; the station once had 12, V-04). Wikipedia's infobox also gives 20 tracks, which has no citation. | `AG18` §4.2 p4; `WP-NDLS` infobox, history ref 16 | secondary |
| V-39 | The platforms are used by direction: platforms **1-4 serve north-bound trains, 5-11 serve passing trains, and 12-16 serve east- and south-bound trains**. Stated as the limit on re-timing trains in the staggering strategy. Year: 2016-2018. | `AG18` §7.1 p9 | secondary |
| V-40 | Platform 5 is the busiest at an average **23 trains per day**, then platform 3 at **22**. 2016, from the berthing chart. Wikipedia's "13-23 trains per platform per day" has its top end here but cites a RISD thesis (ref 37) I did not read, so the 13 is untraced. | `AG18` §5.2.1 p5; `WP-NDLS` | secondary |
| V-41 | Platform counts set against `DM16`: 4 north-bound, 5 east- and south-bound and 7 passing platforms (V-39), against 51 N arrivals, 102 SE arrivals and 73 crossing trains (V-22, V-24). SE arrivals are 66.7% of the total but only 5 of 16 platforms are SE-bound, so the passing platforms must take part of them; the paper does not say how. This is my reading of two counts, not a stated rule. | `AG18`, `DM16` | derived |
| V-42 | Which train types or numbers use which platform, how many of the 16 are in use at any hour, and platform lengths. Looked for in `AG18` (it used the Northern Railway "Platform Berthing Chart" but prints none of it) and in `DM16` (no platform field at all). The chart is held by NDLS station management. | `AG18` p3, `DM16` | UNSOURCED |

---

## Proposed starting values

These are the numbers for slice B1 part 2 to start from. **Every one is assumed, none is solid**: the only per-day source is one 2016 paper, tied to a berthing chart nobody here can see, and the direction and type counts are from a 2016 scrape with no days-of-run. The game should print its traffic as "about", never as a count.

| For | Value | Rests on | Solid or assumed |
|---|---|---|---|
| Trains per day | **275** on a busy day (87 originating, 87 terminating and 103 through; use the shares 31.4/31.4/37.2% so the parts add to 275, V-03); 245 as a quiet-day scale. As a ceiling check, 351 is the number of distinct trains in the whole timetable (V-01), not a day. | V-01, V-02, V-03, V-04, V-12 | **Assumed.** One secondary source, 2016. Wikipedia's 400 and 250 do not trace (V-06). The game should keep a single scale knob for traffic volume. |
| Peak-hour trains | **22** in the busiest hour (09:00), **21** at 17:00 and **20** at 06:00; use the 24-value series V-12 as the hourly weights. About one train every 2.7 minutes in the peak hour. Quietest: 0 at 02:00. | V-12, V-13, V-14 | **Assumed.** Read from a chart image whose labels sum correctly (275), but `DM16` peaks elsewhere (13:00, V-19). |
| North share | **1/3 north, 2/3 south and east**, for arrivals and departures alike; 73 of 233 trains cross the station, none reverses (V-24). Platforms 1-4 north, 5-11 passing, 12-16 south and east (V-39). | V-22, V-23, V-24, V-39 | **Assumed.** Derived from one 2016 timetable counted as daily, grouped by the stop next to NDLS. No official split found (V-26). |
| Type mix | **28% EMU and local, 41% express (SF, Mail, Express, Sampark Kranti, Garib Rath), 31% premium (Rajdhani, Shatabdi, Duronto)**; split the 31% 14.8 / 11.6 / 4.6 in the 32:25:10 ratio of V-27 to V-29. Vande Bharat **0 until sourced**; when added, take its share out of the premium 31%. No freight and no light engines. | V-27 to V-33, V-34, V-36, V-37 | **Assumed.** 28% and 41% are `AG18`'s; reading the leftover 31% as premium trains is my construction, supported only by `DM16`'s 28.8% (67 of 233) for the same three types. Freight and Vande Bharat shares are unsourced. |
| Platform dwell (extra) | Median **20 min**, quartiles 10 and 30, for trains that stop and leave; originating trains need a separate rake-wait value, which no source gives. | V-20 | **Assumed.** 69 trains, 2016 timetable. |

**What the next step should do.** Ask the user whether NDLS station management, Northern Railway or an NTES export can supply the current berthing chart; it would replace V-01 to V-03, V-12 and V-39 to V-42 in one step. Until then the proposed values above stand.
