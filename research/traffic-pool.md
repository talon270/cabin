# traffic-pool.md — the fact ledger for the NDLS train pool

Written 2026-11-15 for cabin slice B1 part 2.
Method: every train number, name, route and time below was parsed by script from the page for that train number, fetched on 2026-11-15; nothing was written from memory. Coach counts come from a second site's coach-position list, lengths from an RDSO specification and a coach slide deck, platforms from the first site's platform field. The 2016 datameet files were downloaded into their own directory under the session scratchpad and read with `python3 -I`; they feed the cross-checks only. No official timetable (NTES, IRCTC, Trains at a Glance) could be read: NTES served a "technical activity" redirect and the Railway Board site served "Site Under Construction", so **the pool is a timetable aggregator, not an Indian Railways page, and every pool row is `secondary` at best.**

**The game uses only lines from this file.** One row is one fact, except pool rows, which hold one source line. Row ids are `P-001` onward. Nothing in the repository was edited.

## Sources

| ID | Document | Status |
|---|---|---|
| `RY` | RailYatri train pages, `railyatri.in/time-table/<number>`: 197 numbers fetched with `curl`, 176 carried a schedule, 139 have an NDLS stop. The schedule is the JSON embedded in each page (`__NEXT_DATA__`, key `trainScheduleDetail`): per stop `sta_min`, `std_min`, `halt`, `platform_number`, plus `updated_at` for the record. A commercial aggregator; it does not say where its data comes from. | Read 2026-11-15. Record dates run 2017 to 2026-11; 66 are from 2026, 29 from 2025, 16 from 2024 and 28 from 2017-2021 (6 Passenger, 22 EMU/MEMU). `secondary`. |
| `IX` | ixigo train pages, `ixigo.com/trains/<number>`: the "coach position" list for the 139 trains. Page footnote: "Coach position info is based on historic data. It does not represent current status". | Read 2026-11-15. Lists an engine entry plus passenger coaches; omits generator cars (C section). `secondary`. |
| `DM` | datameet/railways, `trains.json` (14.8 MB), `schedules.json` (82 MB), 2016, no licence file, raw from github.com/datameet/railways. `stations.json` was also downloaded and not used. | Read by script. **2016: train numbers may have changed since**; section B reports how many did. `secondary`. |
| `WP-RAJ`, `WP-SHT`, `WP-DUR`, `WP-HUM`, `WP-TEJ`, `WP-GR` | Wikipedia: Rajdhani Express, Shatabdi Express, Duronto Express, Humsafar Express, Tejas Express, Garib Rath Express. | Read 2026-11-15 (page text, via the `obscura` CLI). Used to check that pool numbers appear on the type page. `secondary`. |
| `WP-VB`, `WP-VBT` | Wikipedia: Vande Bharat Express (service table "as of July 2026"), and Vande Bharat (trainset). | Read 2026-11-15. `secondary`. |
| `WP-NDLS`, `WP-DSR` | Wikipedia: New Delhi railway station; Delhi Suburban Railway. | Read 2026-11-15. `secondary`. |
| `WP-WAP7`, `WP-WAP5`, `WP-WAP4`, `WP-WAG9` | Wikipedia "Indian locomotive class" articles, infobox dimensions only. | Read 2026-11-15. `WP-WAG12` was also fetched and not used. `secondary`. |
| `PIB-1966347` | PIB, Ministry of Railways, "Vande Bharat Express: A Make in India Success Story", posted 10 Oct 2023, PIB Ahmedabad. | Read 2026-11-15 (HTML, text search for coach and seat figures; not every paragraph). `primary`. The tool-based fetch returned 403; `curl` worked. |
| `RDSO-BS` | RDSO, schedule of technical requirements for dead-end energy absorption systems (buffer stops), Spec. RDSO/2016/CG Rev-01, 16 PDF pages, `rdso.indianrailways.gov.in/uploads/Final draft spec(1).pdf`. | Read 2026-11-15 (the passenger-train limits table and the coach-type list). `primary`. A draft-spec file: its figures are limits for a buffer-stop design, not a coach drawing. |
| `IRIMEE-LHB` | IRIMEE (Jamalpur) RSKR library, "LHB Coach Salient Features", 98 slides, `rskr.irimee.in`. | Read 2026-11-15 (the "major features" slide). Training slides, not an issued document: `secondary`. |
| `IRIMEE-MAINT` | IRIMEE RSKR library, "Maintenance pattern of coaching trains", slides reproducing Railway Board letter 95/M(C)/141/1 Pt of 14.06.2017. | Read 2026-11-15 (categories 1, 1A, 2, 3(a)(i), 3(a)(ii)). `secondary`: the letter itself was not opened. |
| `KRCL-NOTICE` | Konkan Railway Corporation, "Permanent Augmentation of Coaches in Rajdhani Trains", 1 Feb 2025, `konkanrailway.com/en/node/6070`. | Read 2026-11-15 through the fetch tool, which returned an extract with partial quotes. `secondary`. |
| `LS-REPLIES` | Lok Sabha and Rajya Sabha answers on Mail/Express composition (Q.3745 Dec 2024, Q.2813 Aug 2025, RS Q.607 Jul 2024) and on Rajdhani/Shatabdi coach counts. | **Not available.** `eparlib.nic.in` did not resolve, `eparlib.sansad.in` timed out or refused, and `rsdebate.nic.in` returned a JavaScript shell instead of the PDF. Only search-result excerpts were seen. |
| `NTES`, `RB` | NTES (`enquiry.indianrail.gov.in`), Indian Railways timetable "Trains at a Glance", Railway Board (`indianrailways.gov.in/railwayboard`). | **Not available.** NTES redirected to a "technical activity" page; the Railway Board file URL returned "Site Under Construction"; no current Trains at a Glance PDF was found. |
| not used | `etrain.info`, `railmitra.com`, `indiarailinfo.com` station and train pages, and `en.wikipedia.org/wiki/List_of_*_trains`. | Opened; the first three served an empty shell to the fetcher, and the Wikipedia list pages returned 404 or a stub. Nothing cites them. |

**How to read Conf.** `primary` — in an issued Indian Railways, RDSO or PIB document I read. `secondary` — from an aggregator, Wikipedia or training slides. `derived` — counted or computed by me from a named source; the row says how. `unconfirmed` — a search excerpt only, a stale record, or two sources that disagree. `UNSOURCED` — looked for and not found.

---

## A. The train pool at NDLS (question 1)

One row per train, all from one source line: the `RY` page for that number, read 2026-11-15. **Type is derived** from the train name and the page's `train_type` field, which disagrees with the name for some trains (12273 Duronto is typed MAIL EXPRESS), so the name wins. `rec` is the date the page says the record was last updated. `PF` is the platform field on the same page: its origin is not stated (see section E). Rows with a record older than 2024 are `unconfirmed`: 28 of 139 are from 2017-2021 (6 Passenger, 22 EMU/MEMU).

| ID | Claim | Source | Conf |
|---|---|---|---|
| P-001 | 12301 Howrah - New Delhi Rajdhani Express (Via Gaya) · Rajdhani · HWH→NDLS · terminates, arr 10:05 · PF 14 | RY /time-table/12301, rec 2026-2-16 | secondary |
| P-002 | 12302 New Delhi - Howrah Rajdhani Express (Via Gaya) · Rajdhani · NDLS→HWH · starts, dep 16:50 · PF 14 | RY /time-table/12302, rec 2026-2-16 | secondary |
| P-003 | 12305 Howrah - New Delhi Rajdhani Express (Via Patna) · Rajdhani · HWH→NDLS · terminates, arr 10:05 · PF 14 | RY /time-table/12305, rec 2026-2-14 | secondary |
| P-004 | 12306 New Delhi - Howrah Rajdhani Express (Via Patna) · Rajdhani · NDLS→HWH · starts, dep 16:50 · PF 14 | RY /time-table/12306, rec 2026-2-16 | secondary |
| P-005 | 12309 Rajendra Nagar T - New Delhi Tejas Rajdhani Express · Rajdhani · RJPB→NDLS · terminates, arr 07:55 · PF 7 | RY /time-table/12309, rec 2026-10-28 | secondary |
| P-006 | 12310 New Delhi - Rajendra Nagar T Tejas Rajdhani Express · Rajdhani · NDLS→RJPB · starts, dep 17:10 · PF 9 | RY /time-table/12310, rec 2026-2-16 | secondary |
| P-007 | 12313 Sealdah - New Delhi Rajdhani Express · Rajdhani · SDAH→NDLS · terminates, arr 10:50 · PF 13 | RY /time-table/12313, rec 2025-12-31 | secondary |
| P-008 | 12314 New Delhi - Sealdah Rajdhani Express · Rajdhani · NDLS→SDAH · starts, dep 16:30 · PF 13 | RY /time-table/12314, rec 2025-12-31 | secondary |
| P-009 | 12423 Dibrugarh - New Delhi Rajdhani Express · Rajdhani · DBRG→NDLS · terminates, arr 10:30 · PF 15 | RY /time-table/12423, rec 2026-8-14 | secondary |
| P-010 | 12424 New Delhi - Dibrugarh Rajdhani Express · Rajdhani · NDLS→DBRG · starts, dep 16:20 · PF 15 | RY /time-table/12424, rec 2026-8-12 | secondary |
| P-011 | 12425 New Delhi - Jammu Tawi Rajdhani Express · Rajdhani · NDLS→JAT · starts, dep 20:40 · PF 12 | RY /time-table/12425, rec 2026-7-27 | secondary |
| P-012 | 12426 Jammu Tawi - New Delhi Rajdhani Express · Rajdhani · JAT→NDLS · terminates, arr 05:55 · PF 15 | RY /time-table/12426, rec 2026-7-27 | secondary |
| P-013 | 12441 Bilaspur - New Delhi Rajdhani Express · Rajdhani · BSP→NDLS · terminates, arr 10:40 · PF 13 | RY /time-table/12441, rec 2025-12-31 | secondary |
| P-014 | 12442 New Delhi - Bilaspur Rajdhani Express · Rajdhani · NDLS→BSP · starts, dep 15:25 · PF 15 | RY /time-table/12442, rec 2024-12-31 | secondary |
| P-015 | 12453 Ranchi - New Delhi Rajdhani Express (via Chopan) · Rajdhani · RNC→NDLS · terminates, arr 11:10 · PF 15 | RY /time-table/12453, rec 2026-5-16 | secondary |
| P-016 | 12454 New Delhi - Ranchi Rajdhani Express (via Chopan) · Rajdhani · NDLS→RNC · starts, dep 16:10 · PF 14 | RY /time-table/12454, rec 2026-5-16 | secondary |
| P-017 | 12951 Mumbai Central - New Delhi Rajdhani Express · Rajdhani · MMCT→NDLS · terminates, arr 08:32 · PF 3 | RY /time-table/12951, rec 2025-12-31 | secondary |
| P-018 | 12952 New Delhi - Mumbai Central Rajdhani Express · Rajdhani · NDLS→MMCT · starts, dep 16:55 · PF 3 | RY /time-table/12952, rec 2025-12-31 | secondary |
| P-019 | 12957 Swarna Jayanti Rajdhani Express · Rajdhani · SBIB→NDLS · terminates, arr 07:30 · PF 1 | RY /time-table/12957, rec 2025-11-24 | secondary |
| P-020 | 12958 Swarna Jayanti Rajdhani Express · Rajdhani · NDLS→SBIB · starts, dep 19:55 · PF 1 | RY /time-table/12958, rec 2026-3-24 | secondary |
| P-021 | 20839 Ranchi - New Delhi Rajdhani Express (via Bokaro Steel City) · Rajdhani · RNC→NDLS · terminates, arr 11:10 · PF 14 | RY /time-table/20839, rec 2026-1-1 | secondary |
| P-022 | 20840 New Delhi - Ranchi Rajdhani Express (Via Bokaro Steel City) · Rajdhani · NDLS→RNC · starts, dep 16:10 · PF 14 | RY /time-table/20840, rec 2026-1-1 | secondary |
| P-023 | 22811 Bhubaneswar - New Delhi Tejas Rajdhani Express (via Adra) · Rajdhani · BBS→NDLS · terminates, arr 09:55 · PF 11 | RY /time-table/22811, rec 2026-8-10 | secondary |
| P-024 | 22812 New Delhi - Bhubaneswar Tejas Rajdhani Express (via Adra) · Rajdhani · NDLS→BBS · starts, dep 17:00 · PF 11 | RY /time-table/22812, rec 2025-12-31 | secondary |
| P-025 | 22823 Bhubaneswar - New Delhi Tejas Rajdhani Express (via Tatanagar) · Rajdhani · BBS→NDLS · terminates, arr 09:55 · PF 11 | RY /time-table/22823, rec 2026-8-8 | secondary |
| P-026 | 22824 New Delhi - Bhubaneswar Tejas Rajdhani Express (Via Tatanagar) · Rajdhani · NDLS→BBS · starts, dep 17:00 · PF 11 | RY /time-table/22824, rec 2026-8-6 | secondary |
| P-027 | 12001 Shatabdi Express · Shatabdi · RKMP→NDLS · terminates, arr 23:50 · PF 1 | RY /time-table/12001, rec 2026-10-18 | secondary |
| P-028 | 12002 Shatabdi Express · Shatabdi · NDLS→RKMP · starts, dep 06:00 · PF 1 | RY /time-table/12002, rec 2026-10-18 | secondary |
| P-029 | 12003 Lucknow Jn - New Delhi Swarn Shatabdi Express · Shatabdi · LJN→NDLS · terminates, arr 22:25 · PF 8 | RY /time-table/12003, rec 2025-7-20 | secondary |
| P-030 | 12004 New Delhi - Lucknow Jn. Swarn Shatabdi Express · Shatabdi · NDLS→LJN · starts, dep 06:10 · PF 9 | RY /time-table/12004, rec 2026-5-12 | secondary |
| P-031 | 12005 Shatabdi Express · Shatabdi · NDLS→KLK · starts, dep 17:15 · PF 1 | RY /time-table/12005, rec 2024-3-8 | secondary |
| P-032 | 12006 Shatabdi Express · Shatabdi · KLK→NDLS · terminates, arr 10:15 · PF 1 | RY /time-table/12006, rec 2024-6-21 | secondary |
| P-033 | 12011 Shatabdi Express · Shatabdi · NDLS→KLK · starts, dep 07:40 · PF 2 | RY /time-table/12011, rec 2024-3-8 | secondary |
| P-034 | 12012 Shatabdi Express · Shatabdi · KLK→NDLS · terminates, arr 21:50 · PF 1 | RY /time-table/12012, rec 2024-3-8 | secondary |
| P-035 | 12013 Shatabdi Express · Shatabdi · NDLS→ASR · starts, dep 16:30 · PF 1 | RY /time-table/12013, rec 2024-3-8 | secondary |
| P-036 | 12014 Shatabdi Express · Shatabdi · ASR→NDLS · terminates, arr 11:02 · PF 1 | RY /time-table/12014, rec 2025-1-4 | secondary |
| P-037 | 12015 New Delhi - Daurai (Ajmer) Shatabdi Express · Shatabdi · NDLS→DOZ · starts, dep 06:10 · PF 10 | RY /time-table/12015, rec 2026-11-4 | secondary |
| P-038 | 12016 Daurai (Ajmer) - New Delhi Shatabdi Express · Shatabdi · DOZ→NDLS · terminates, arr 22:30 · PF 4 | RY /time-table/12016, rec 2026-5-16 | secondary |
| P-039 | 12017 New Delhi - Dehradun Shatabdi Express · Shatabdi · NDLS→DDN · starts, dep 06:45 · PF 16 | RY /time-table/12017, rec 2025-12-11 | secondary |
| P-040 | 12018 Dehradun - New Delhi Shatabdi Express · Shatabdi · DDN→NDLS · terminates, arr 22:50 · PF 11 | RY /time-table/12018, rec 2025-12-31 | secondary |
| P-041 | 12029 Swarna Jayanti Shatabdi Express · Shatabdi · NDLS→ASR · starts, dep 07:20 · PF 1 | RY /time-table/12029, rec 2025-1-9 | secondary |
| P-042 | 12030 Swarna Jayanti Shatabdi Express · Shatabdi · ASR→NDLS · terminates, arr 22:50 · PF 3 | RY /time-table/12030, rec 2025-1-10 | secondary |
| P-043 | 12031 Shatabdi Express · Shatabdi · NDLS→ASR · starts, dep 07:20 · PF 1 | RY /time-table/12031, rec 2024-3-8 | secondary |
| P-044 | 12032 Shatabdi Express · Shatabdi · ASR→NDLS · terminates, arr 22:50 · PF 1 | RY /time-table/12032, rec 2024-3-8 | secondary |
| P-045 | 12033 Kanpur Central - New Delhi Shatabdi Express · Shatabdi · CNB→NDLS · terminates, arr 11:20 · PF 2 | RY /time-table/12033, rec 2025-12-31 | secondary |
| P-046 | 12034 New Delhi - Kanpur Central Shatabdi Express · Shatabdi · NDLS→CNB · starts, dep 15:40 · PF 10 | RY /time-table/12034, rec 2025-2-10 | secondary |
| P-047 | 12039 Shatabdi Express · Shatabdi · KGM→NDLS · terminates, arr 20:55 · PF 2 | RY /time-table/12039, rec 2026-1-24 | secondary |
| P-048 | 12040 Shatabdi Express · Shatabdi · NDLS→KGM · starts, dep 06:20 · PF 2 | RY /time-table/12040, rec 2025-12-31 | secondary |
| P-049 | 12045 Shatabdi Express · Shatabdi · NDLS→CDG · starts, dep 19:15 · PF 2 | RY /time-table/12045, rec 2024-6-21 | secondary |
| P-050 | 12046 Shatabdi Express · Shatabdi · CDG→NDLS · terminates, arr 15:20 · PF 2 | RY /time-table/12046, rec 2025-12-31 | secondary |
| P-051 | 12055 Jan Shatabdi Express · Jan Shatabdi · NDLS→DDN · starts, dep 15:20 · PF 9 | RY /time-table/12055, rec 2025-12-31 | secondary |
| P-052 | 12056 Jan Shatabdi Express · Jan Shatabdi · DDN→NDLS · terminates, arr 11:05 · PF 10 | RY /time-table/12056, rec 2024-12-31 | secondary |
| P-053 | 12057 Jan Shatabdi Express · Jan Shatabdi · NDLS→DLPC · starts, dep 14:35 · PF 10 | RY /time-table/12057, rec 2026-10-31 | secondary |
| P-054 | 12058 Jan Shatabdi Express · Jan Shatabdi · DLPC→NDLS · terminates, arr 11:45 · PF 9 | RY /time-table/12058, rec 2026-10-31 | secondary |
| P-055 | 12259 Sealdah - Bikaner AC Duronto Express · Duronto · SDAH→BKN · through, arr 11:00 dep 11:15 (halt 15 min) · PF 9 | RY /time-table/12259, rec 2026-9-28 | secondary |
| P-056 | 12260 Bikaner - Sealdah AC Duronto Express · Duronto · BKN→SDAH · through, arr 19:30 dep 19:45 (halt 15 min) · PF 13 | RY /time-table/12260, rec 2026-9-26 | secondary |
| P-057 | 12273 Howrah - New Delhi Duronto Express · Duronto · HWH→NDLS · terminates, arr 06:55 · PF 8 | RY /time-table/12273, rec 2026-10-30 | secondary |
| P-058 | 12274 New Delhi - Howrah Duronto Express · Duronto · NDLS→HWH · starts, dep 12:35 · PF 5 | RY /time-table/12274, rec 2025-12-31 | secondary |
| P-059 | 12281 Bhubaneswar - New Delhi Duronto Express · Duronto · BBS→NDLS · terminates, arr 06:55 · PF 8 | RY /time-table/12281, rec 2026-10-28 | secondary |
| P-060 | 12282 New Delhi - Bhubaneswar Duronto Express · Duronto · NDLS→BBS · starts, dep 12:35 · PF 5 | RY /time-table/12282, rec 2025-12-31 | secondary |
| P-061 | 22435 Vande Bharat Express · Vande Bharat · BSB→NDLS · terminates, arr 23:00 · PF 16 | RY /time-table/22435, rec 2024-12-17 | secondary |
| P-062 | 22436 Vande Bharat Express · Vande Bharat · NDLS→BSB · starts, dep 06:00 · PF 16 | RY /time-table/22436, rec 2024-10-20 | secondary |
| P-063 | 22439 Vande Bharat Express · Vande Bharat · NDLS→SVDK · starts, dep 06:00 · PF 14 | RY /time-table/22439, rec 2026-3-30 | secondary |
| P-064 | 22440 Vande Bharat Express · Vande Bharat · SVDK→NDLS · terminates, arr 23:10 · PF 14 | RY /time-table/22440, rec 2026-3-30 | secondary |
| P-065 | 22447 Vande Bharat Express · Vande Bharat · NDLS→AADR · starts, dep 05:50 · PF 11 | RY /time-table/22447, rec 2026-7-30 | secondary |
| P-066 | 22448 Vande Bharat Express · Vande Bharat · AADR→NDLS · terminates, arr 18:25 · PF 11 | RY /time-table/22448, rec 2026-7-28 | secondary |
| P-067 | 12275 Prayagraj - New Delhi Humsafar Express · Humsafar · PRYJ→NDLS · terminates, arr 06:35 · PF 5 | RY /time-table/12275, rec 2026-10-28 | secondary |
| P-068 | 12276 New Delhi - Prayagraj Humsafar Express · Humsafar · NDLS→PRYJ · starts, dep 23:05 · PF 9 | RY /time-table/12276, rec 2026-10-28 | secondary |
| P-069 | 12349 Godda - New Delhi Humsafar Express · Humsafar · GODA→NDLS · terminates, arr 12:50 · PF 13 | RY /time-table/12349, rec 2026-2-2 | secondary |
| P-070 | 12350 New Delhi - Godda Humsafar Express · Humsafar · NDLS→GODA · starts, dep 23:45 · PF 15 | RY /time-table/12350, rec 2026-1-6 | secondary |
| P-071 | 82501 Lucknow Jn. - New Delhi IRCTC Tejas Express · Tejas · LJN→NDLS · terminates, arr 12:35 · PF 11 | RY /time-table/82501, rec 2024-12-4 | secondary |
| P-072 | 82502 New Delhi - Lucknow Jn. IRCTC Tejas Express · Tejas · NDLS→LJN · starts, dep 15:30 · PF 11 | RY /time-table/82502, rec 2024-12-4 | secondary |
| P-073 | 12877 Ranchi - New Delhi Garib Rath Express · Garib Rath · RNC→NDLS · terminates, arr 11:10 · PF 15 | RY /time-table/12877, rec 2026-6-26 | secondary |
| P-074 | 12878 New Delhi - Ranchi Garib Rath Express · Garib Rath · NDLS→RNC · starts, dep 16:10 · PF 14 | RY /time-table/12878, rec 2026-6-24 | secondary |
| P-075 | 12229 Lucknow Mail · Superfast · LKO→NDLS · terminates, arr 07:00 · PF 16 | RY /time-table/12229, rec 2026-10-28 | secondary |
| P-076 | 12304 Poorva Express (Via Patna) · Superfast · SSB→HWH · through, arr 17:15 dep 17:40 (halt 25 min) · PF 14 | RY /time-table/12304, rec 2026-10-29 | secondary |
| P-077 | 12391 Shramjeevi SF Express · Superfast · RGD→NDLS · terminates, arr 04:40 · PF 6 | RY /time-table/12391, rec 2026-11-4 | secondary |
| P-078 | 12392 Shramjeevi SF Express · Superfast · NDLS→RGD · starts, dep 13:10 · PF 8 | RY /time-table/12392, rec 2026-5-20 | secondary |
| P-079 | 12397 Mahabodhi SF Express · Superfast · GAYA→NDLS · terminates, arr 04:10 · PF 9 | RY /time-table/12397, rec 2025-12-31 | secondary |
| P-080 | 12398 Mahabodhi SF Express · Superfast · NDLS→GAYA · starts, dep 12:25 · PF 6 | RY /time-table/12398, rec 2025-12-31 | secondary |
| P-081 | 12417 Prayagraj Express · Superfast · PRYJ→NDLS · terminates, arr 07:15 · PF 14 | RY /time-table/12417, rec 2026-10-28 | secondary |
| P-082 | 12418 Prayagraj Express · Superfast · NDLS→PRYJ · starts, dep 22:15 · PF 14 | RY /time-table/12418, rec 2026-10-28 | secondary |
| P-083 | 12419 Gomti Express · Superfast · LKO→NDLS · terminates, arr 15:00 · PF 6 | RY /time-table/12419, rec 2026-5-12 | secondary |
| P-084 | 12420 Gomti Express · Superfast · NDLS→LKO · starts, dep 12:55 · PF 12 | RY /time-table/12420, rec 2025-12-31 | secondary |
| P-085 | 12445 Uttar Sampark Kranti Express · Superfast · NDLS→SVDK · starts, dep 20:50 · PF 15 | RY /time-table/12445, rec 2026-10-31 | secondary |
| P-086 | 12446 Uttar Sampark Kranti Express · Superfast · SVDK→NDLS · terminates, arr 06:55 · PF 15 | RY /time-table/12446, rec 2026-10-30 | secondary |
| P-087 | 12497 Shan-e-Punjab Express · Superfast · NDLS→ASR · starts, dep 06:40 · PF 14 | RY /time-table/12497, rec 2026-10-31 | secondary |
| P-088 | 12498 Shan-e-Punjab Express · Superfast · ASR→NDLS · terminates, arr 22:30 · PF 14 | RY /time-table/12498, rec 2026-10-31 | secondary |
| P-089 | 12555 Gorakhdham SF Express · Superfast · GKP→BTI · through, arr 05:15 dep 05:30 (halt 15 min) · PF 5 | RY /time-table/12555, rec 2026-9-14 | secondary |
| P-090 | 12559 Shiv Ganga SF Express · Superfast · BNRS→NDLS · terminates, arr 08:30 · PF 12 | RY /time-table/12559, rec 2025-12-3 | secondary |
| P-091 | 12560 Shiv Ganga SF Express · Superfast · NDLS→BNRS · starts, dep 20:05 · PF 12 | RY /time-table/12560, rec 2025-12-3 | secondary |
| P-092 | 12561 Swatantrata Senani Express · Superfast · JYG→NDLS · terminates, arr 15:45 · PF 12 | RY /time-table/12561, rec 2025-12-31 | secondary |
| P-093 | 12562 Swatantrata Senani Express · Superfast · NDLS→JYG · starts, dep 21:15 · PF 13 | RY /time-table/12562, rec 2025-12-31 | secondary |
| P-094 | 12565 Bihar Sampark Kranti Express · Superfast · DBG→NDLS · terminates, arr 05:05 · PF 13 | RY /time-table/12565, rec 2026-9-4 | secondary |
| P-095 | 12566 Bihar Sampark Kranti Express · Superfast · NDLS→DBG · starts, dep 12:45 · PF 16 | RY /time-table/12566, rec 2026-8-18 | secondary |
| P-096 | 12615 Grand Trunk Express · Superfast · MAS→NDLS · terminates, arr 05:10 · PF 4 | RY /time-table/12615, rec 2026-10-14 | secondary |
| P-097 | 12616 Grand Trunk Express · Superfast · NDLS→MAS · starts, dep 16:10 · PF 4 | RY /time-table/12616, rec 2026-5-22 | secondary |
| P-098 | 12621 Tamil Nadu Express · Superfast · MAS→NDLS · terminates, arr 06:30 · PF 3 | RY /time-table/12621, rec 2026-10-14 | secondary |
| P-099 | 12622 Tamil Nadu Express · Superfast · NDLS→MAS · starts, dep 21:05 · PF 3 | RY /time-table/12622, rec 2026-10-14 | secondary |
| P-100 | 12625 Kerala SF Express · Superfast · TVC→NDLS · terminates, arr 13:30 · PF 7 | RY /time-table/12625, rec 2026-10-14 | secondary |
| P-101 | 12626 Kerala Express · Superfast · NDLS→TVC · starts, dep 20:10 · PF 3 | RY /time-table/12626, rec 2026-4-14 | secondary |
| P-102 | 12627 Karnataka SF Express · Superfast · SBC→NDLS · terminates, arr 09:00 · PF 7 | RY /time-table/12627, rec 2026-10-14 | secondary |
| P-103 | 12628 Karnataka SF Express · Superfast · NDLS→SBC · starts, dep 20:20 · PF 6 | RY /time-table/12628, rec 2026-8-3 | secondary |
| P-104 | 12724 Telangana Express · Superfast · NDLS→HYB · starts, dep 16:00 · PF 5 | RY /time-table/12724, rec 2026-7-20 | secondary |
| P-105 | 20451 Sogaria - New Delhi Intercity SF Express · Superfast · SGAC→NDLS · terminates, arr 22:35 · PF 12 | RY /time-table/20451, rec 2024-12-31 | secondary |
| P-106 | 20452 New Delhi - Sogaria Intercity SF Express · Superfast · NDLS→SGAC · starts, dep 07:10 · PF 6 | RY /time-table/20452, rec 2024-12-31 | secondary |
| P-107 | 12230 Lucknow Mail · Mail/Express · NDLS→LKO · starts, dep 22:00 · PF 12 | RY /time-table/12230, rec 2025-12-31 | secondary |
| P-108 | 12303 Poorva Express (via Patna) · Mail/Express · HWH→SSB · through, arr 06:20 dep 06:45 (halt 25 min) · PF 12 | RY /time-table/12303, rec 2026-11-3 | secondary |
| P-109 | 12723 Telangana Express · Mail/Express · HYB→NDLS · terminates, arr 08:00 · PF 5 | RY /time-table/12723, rec 2025-12-31 | secondary |
| P-110 | 14211 Intercity Express · Mail/Express · GWL→NDLS · terminates, arr 10:00 · PF 12 | RY /time-table/14211, rec 2026-11-12 | secondary |
| P-111 | 14212 Intercity Express · Mail/Express · NDLS→GWL · starts, dep 17:40 · PF 6 | RY /time-table/14212, rec 2026-11-6 | secondary |
| P-112 | 51901 AGRA CANTT - OLD DELHI Passenger · Passenger · AGC→DLI · through, arr 15:31 dep 15:36 (halt 5 min) · PF 1 | RY /time-table/51901, rec 2019-8-6 | unconfirmed |
| P-113 | 51902 OLD DELHI - AGRA CANTT Passenger · Passenger · DLI→AGC · through, arr 07:15 dep 07:17 (halt 2 min) · PF 7 | RY /time-table/51902, rec 2019-8-6 | unconfirmed |
| P-114 | 54473 OLD DELHI - SAHARANPUR Passenger · Passenger · DLI→SRE · through, arr 18:33 dep 18:40 (halt 7 min) · PF 15 | RY /time-table/54473, rec 2019-8-9 | unconfirmed |
| P-115 | 54474 SAHARANPUR - OLD DELHI Passenger · Passenger · SRE→DLI · through, arr 08:25 dep 08:27 (halt 2 min) · PF 11 | RY /time-table/54474, rec 2019-8-9 | unconfirmed |
| P-116 | 54539 HAZRAT NIZAMUDDIN - AMBALA CANTT Passenger · Passenger · NZM→UMB · through, arr 03:20 dep 03:30 (halt 10 min) · PF 7 | RY /time-table/54539, rec 2019-8-9 | unconfirmed |
| P-117 | 54540 AMBALA CANTT - HAZRAT NIZAMUDDIN Passenger · Passenger · UMB→NZM · through, arr 00:30 dep 00:40 (halt 10 min) · PF 1 | RY /time-table/54540, rec 2019-8-9 | unconfirmed |
| P-118 | 04471 GHAZIABAD - PANIPAT MEMU Express Special (UnReserved) · EMU/MEMU · GZB→PNP · through, arr 18:05 dep 18:07 (halt 2 min) · PF 6 | RY /time-table/04471, rec 2021-4-6 | unconfirmed |
| P-119 | 04472 PANIPAT - GHAZIABAD MEMU Express Special (UnReserved) · EMU/MEMU · PNP→GZB · through, arr 08:16 dep 08:18 (halt 2 min) · PF 10 | RY /time-table/04472, rec 2021-4-6 | unconfirmed |
| P-120 | 64062 OLD DELHI - PALWAL Local · EMU/MEMU · DLI→PWL · through, arr 05:33 dep 05:35 (halt 2 min) · PF 6 | RY /time-table/64062, rec 2018-1-10 | unconfirmed |
| P-121 | 64064 OLD DELHI - PALWAL Local · EMU/MEMU · DLI→PWL · through, arr 20:04 dep 20:06 (halt 2 min) · PF 6 | RY /time-table/64064, rec 2018-1-10 | unconfirmed |
| P-122 | 64074 KOSI KALAN - NEW DELHI Local · EMU/MEMU · NDLS→KSV · starts, dep 04:45 · PF 4 | RY /time-table/64074, rec 2018-1-10 | unconfirmed |
| P-123 | 64076 NEW DELHI - PALWAL Local · EMU/MEMU · NDLS→PWL · starts, dep 00:15 · PF 8 | RY /time-table/64076, rec 2018-1-10 | unconfirmed |
| P-124 | 64078 NEW DELHI - PALWAL Local · EMU/MEMU · NDLS→PWL · starts, dep 08:45 · PF 2 | RY /time-table/64078, rec 2018-11-29 | unconfirmed |
| P-125 | 64080 NEW DELHI - PALWAL Local · EMU/MEMU · NDLS→PWL · starts, dep 17:05 · PF 5 | RY /time-table/64080, rec 2018-1-10 | unconfirmed |
| P-126 | 64084 NEW DELHI - PALWAL Local · EMU/MEMU · NDLS→PWL · starts, dep 21:25 · PF 2 | RY /time-table/64084, rec 2018-11-29 | unconfirmed |
| P-127 | 64089 HAZRAT NIZAMUDDIN Parikrama Local · EMU/MEMU · NZM→HNZM · through, arr 18:03 dep 18:05 (halt 2 min) · PF 1 | RY /time-table/64089, rec 2018-1-10 | unconfirmed |
| P-128 | 64095 NEW DELHI - SHAKURBASTI Local · EMU/MEMU · NDLS→SSB · starts, dep 09:50 · PF 5 | RY /time-table/64095, rec 2017-2-9 | unconfirmed |
| P-129 | 64449 GHAZIABAD - NEW DELHI Ladies Special Local · EMU/MEMU · GZB→NDLS · terminates, arr 09:20 · PF 15 | RY /time-table/64449, rec 2018-1-11 | unconfirmed |
| P-130 | 64450 NEW DELHI - GHAZIABAD Ladies Special Local · EMU/MEMU · NDLS→GZB · starts, dep 17:50 · PF 5 | RY /time-table/64450, rec 2018-1-11 | unconfirmed |
| P-131 | 64464 PANIPAT - NEW DELHI MEMU · EMU/MEMU · PNP→NDLS · terminates, arr 07:30 · PF 12 | RY /time-table/64464, rec 2019-8-20 | unconfirmed |
| P-132 | 64466 PANIPAT - NEW DELHI MEMU · EMU/MEMU · PNP→NDLS · terminates, arr 23:55 · PF 1 | RY /time-table/64466, rec 2019-8-20 | unconfirmed |
| P-133 | 64471 GHAZIABAD - PANIPAT MEMU · EMU/MEMU · GZB→PNP · through, arr 18:03 dep 18:05 (halt 2 min) · PF 6 | RY /time-table/64471, rec 2019-8-20 | unconfirmed |
| P-134 | 64472 PANIPAT - GHAZIABAD MEMU · EMU/MEMU · PNP→GZB · through, arr 08:16 dep 08:18 (halt 2 min) · PF 10 | RY /time-table/64472, rec 2019-8-20 | unconfirmed |
| P-135 | 64491 PALWAL - NEW DELHI Ladies Special Local · EMU/MEMU · PWL→NDLS · terminates, arr 10:00 · PF 5 | RY /time-table/64491, rec 2018-1-11 | unconfirmed |
| P-136 | 64492 NEW DELHI - PALWAL Ladies Special Local · EMU/MEMU · NDLS→PWL · starts, dep 17:50 · PF 3 | RY /time-table/64492, rec 2018-1-11 | unconfirmed |
| P-137 | 64534 PANIPAT - GHAZIABAD MEMU · EMU/MEMU · PNP→GZB · through, arr 19:55 dep 20:05 (halt 10 min) · PF 11 | RY /time-table/64534, rec 2019-8-20 | unconfirmed |
| P-138 | 64911 NEW DELHI - ROHTAK MEMU · EMU/MEMU · NDLS→ROK · starts, dep 09:25 · PF 1 | RY /time-table/64911, rec 2019-8-20 | unconfirmed |
| P-139 | 64912 ROHTAK - NEW DELHI MEMU · EMU/MEMU · ROK→NDLS · terminates, arr 09:15 · PF 1 | RY /time-table/64912, rec 2019-8-20 | unconfirmed |

## B. The 2016 datameet data, and how much of it still holds

| ID | Claim | Source | Conf |
|---|---|---|---|
| P-140 | datameet `trains.json` holds 5,208 trains; `schedules.json` holds 417,080 rows, of which 233 are at station code NDLS; counted by script with `python3 -I`. 2016 data, no licence file. **No pool row above uses it**; it was used only for the cross-checks below. | DM (counted) | derived |
| P-141 | Of the 121 train numbers that datameet lists at NDLS and that I also fetched from `RY`, 107 still list NDLS and 14 no longer do: 12191, 12192, 12203, 12204, 12401, 12402, 12501, 12502, 12801, 12802, 12819, 12820, 12825, 12826. A pool built from datameet alone would be 12% dead on the first check. | DM + RY, compared by script | derived |
| P-142 | Of those 107 still at NDLS, 73 have the same origin and destination station code in both; 34 do not (renamed, extended or renumbered). **Number plus name plus route held for 73 of 107 (68%).** | DM + RY, compared by script | derived |
| P-143 | Of those 73, the NDLS time agrees within 30 min for 55, within 10 min for 33 and exactly for 13. Datameet times are 2016 and should not be used as the schedule. | DM + RY, compared by script | derived |
| P-144 | Number still matches name: 12301 Howrah-New Delhi Rajdhani (HWH→NDLS); 12013 New Delhi-Amritsar Shatabdi (NDLS→ASR, dep 16:30 in both); 12497 Shan-e-Punjab (NDLS→ASR, dep 06:40 in both); 12621 Tamil Nadu Express (MAS→NDLS); 12877 Ranchi-New Delhi Garib Rath. These five held; the next five rows are cases where it did not. | DM trains.json; RY pages 12301, 12013, 12497, 12621, 12877 | derived |
| P-145 | Number now means a different train: 12501 is "Guwahati - New Delhi Poorvottar Sampark Kranti" in datameet and "Kolkata - Agartala Garib Rath Express" on `RY`, with no NDLS stop. | DM trains.json; RY /time-table/12501 | derived |
| P-146 | Type changed: 12275/12276 are "Allahabad-New Delhi Duronto" in datameet (12276 dep NDLS 23:00) and "Prayagraj - New Delhi Humsafar Express" on `RY` (12276 dep 23:05). | DM; RY /time-table/12276 | derived |
| P-147 | Name changed: 12723 is "Andhra Pradesh Express" in datameet and "Telangana Express" on `RY`. | DM; RY /time-table/12723 | derived |
| P-148 | Route changed: 12259 is Sealdah-New Delhi in datameet and Sealdah - Bikaner on `RY`, now a through stop at NDLS (arr 11:00, dep 11:15). | DM; RY /time-table/12259 | derived |
| P-149 | Route changed: 12349 is Bhagalpur-New Delhi in datameet and Godda - New Delhi Humsafar on `RY`. | DM; RY /time-table/12349 | derived |
| P-150 | Independent check of the pool against a second current page: every pool number of these types also appears in the Wikipedia type article: Rajdhani 26 of 26, Duronto 6 of 6, Humsafar 4 of 4, Tejas 2 of 2, Garib Rath 2 of 2 (printed as "12877/78"), Shatabdi 24 of 24. Names were not compared character by character. Jan Shatabdi (4) and Superfast/Mail/Passenger/EMU were not checked this way. | WP-RAJ, WP-SHT, WP-DUR, WP-HUM, WP-TEJ, WP-GR; matched by script | derived |
| P-151 | Vande Bharat has no number in the Wikipedia article. The 6 VB numbers (22435, 22436, 22439, 22440, 22447, 22448) rest on `RY` alone; the routes agree with the Wikipedia table (New Delhi–Varanasi, –SMVD Katra, –Amb Andaura). | WP-VB; RY | unconfirmed |
| P-152 | Trains leaving NDLS: a 2025-06-16 Amar Ujala headline reads "these trains including Purushottam express, Mumbai Duronto will no longer run on New Delhi station". Seen as a search-result title only, not opened. It fits 12801/12802 (Purushottam) having no NDLS stop on `RY`. | WebSearch result title, amarujala.com, 2025-06-16 | unconfirmed |

## C. Coach length, coach count and rake length (question 2)

| ID | Claim | Source | Conf |
|---|---|---|---|
| P-153 | LHB coach: length over body 23,540 mm. The slide prints a second figure in brackets, 21,337 mm, with no label; it reads as the ICF coach. | IRIMEE-LHB, "LHB Coach Design, major features" slide | unconfirmed |
| P-154 | RDSO spec for buffer stops, table of passenger-train limits: maximum length of coach 24,000 mm, width 3,660 mm, height 4,381 mm. | RDSO-BS §(a) | primary |
| P-155 | Coupler projection from the end of the coach: LHB centre buffer coupler 230 mm; ICF centre buffer coupler 480 mm; "SPC" of Vande Bharat 450 mm (the spec does not expand SPC); EMU/MEMU/DEMU side buffers 560 mm. | RDSO-BS coach-type list | primary |
| P-156 | **An LHB vehicle is 24.0 m over couplers, not 22 m:** 23.540 + 2 × 0.230 = 24.000 m, which equals the RDSO maximum length of 24,000 mm. | derived from the two rows above | derived |
| P-157 | An ICF coach is about 22.3 m over couplers: 21.337 + 2 × 0.480 = 22.297 m. This depends on reading the bracketed slide figure as ICF, so it is the weaker of the two lengths. | derived from the two rows above | derived |
| P-158 | Vande Bharat trainset: car length 24 m; train length 384 m for 16 cars. The article gives 192 m for 8 cars and 480 m for 20 cars. | WP-VBT infobox; WP-VB | secondary |
| P-159 | PIB: "The Vande Bharat Express has 16 air-conditioned coaches of which two are executive class coaches. The total seating capacity is 1,128 passengers." | PIB-1966347, 10 Oct 2023 | primary |
| P-160 | Cars per Vande Bharat service touching New Delhi: New Delhi–Varanasi 20, New Delhi–SMVD Katra 20, New Delhi–Amb Andaura 16, Varanasi–New Delhi 20, SMVD Katra–New Delhi 20. 8-car services in the table (Bikaner–Delhi Cantt, Firozpur Cantt–Delhi Jn, Ahmedabad–Okha) do not serve NDLS. | WP-VB service table | secondary |
| P-161 | Rajdhani 12431/12432 (Hazrat Nizamuddin–Thiruvananthapuram) went from 20 to 22 LHB coaches from 4-6 Feb 2025: 2 First AC, 5 2-tier AC, 12 3-tier AC, 1 pantry car, 2 generator cars. The notice names no NDLS train. Read through a fetch-tool extract, not verbatim. | KRCL-NOTICE, 1 Feb 2025 | secondary |
| P-162 | The coach-position list on `IX` begins with an engine entry ("EN") and then the passenger coaches. Vande Bharat 22447 shows EN + 16 coaches and 22436 shows EN + 20, matching Wikipedia's 16 and 20. **Passenger coaches = the page's "has N coaches" minus 1.** The list omits generator cars: 12301 lists 20 passenger coaches, and the Rajdhani notice above counts 20 passenger + 2 generator for its sister train. | IX /trains/12301, /22447, /22436; KRCL-NOTICE | derived |
| P-163 | Passenger coaches on the `IX` list, Rajdhani: 16-21, mode 20 (20 of 26); the 4 at 16 are the Tejas-rake Rajdhanis 22811/22812/22823/22824. | IX /trains/<number> for the 26 Rajdhani rows (P-001 to P-026); counted by script | derived |
| P-164 | Passenger coaches on the `IX` list, Shatabdi: 11-18, mode 16 (12 of 24); 18 for 12003/12004 Swarna Lucknow and 12029-12032 Amritsar. | IX /trains/<number> for the 24 Shatabdi rows (P-027 to P-050); counted by script | derived |
| P-165 | Passenger coaches on the `IX` list, Jan Shatabdi: 14 (4 of 4). | IX /trains/<number> for the 4 Jan Shatabdi rows (P-051 to P-054); counted by script | derived |
| P-166 | Passenger coaches on the `IX` list, Duronto: 18-21 (6 trains: 18, 18, 19, 19, 21, 21). | IX /trains/<number> for the 6 Duronto rows (P-055 to P-060); counted by script | derived |
| P-167 | Passenger coaches on the `IX` list, Vande Bharat: 20 (4 trains), 16 (2 trains: 22447, 22448). | IX /trains/<number> for the 6 Vande Bharat rows (P-061 to P-066); counted by script | derived |
| P-168 | Passenger coaches on the `IX` list, Humsafar: 20 (4 of 4). | IX /trains/<number> for the 4 Humsafar rows (P-067 to P-070); counted by script | derived |
| P-169 | Passenger coaches on the `IX` list, Tejas: 14 (82501, 82502); Wikipedia describes the CSMT-Karmali Tejas as 14 non-executive plus 2 executive chair cars, a different train. | IX /trains/<number> for the 2 Tejas rows (P-071 to P-072); counted by script | derived |
| P-170 | Passenger coaches on the `IX` list, Garib Rath: 18 (2 of 2). | IX /trains/<number> for the 2 Garib Rath rows (P-073 to P-074); counted by script | derived |
| P-171 | Passenger coaches on the `IX` list, Superfast: 19-22, mode 20 (25 of 32). | IX /trains/<number> for the 32 Superfast rows (P-075 to P-106); counted by script | derived |
| P-172 | Passenger coaches on the `IX` list, Mail/Express: 20-21 (5 trains). | IX /trains/<number> for the 5 Mail/Express rows (P-107 to P-111); counted by script | derived |
| P-173 | Passenger coaches on the `IX` list, Passenger: 9-15 (6 trains; records from 2019). | IX /trains/<number> for the 6 Passenger rows (P-112 to P-117); counted by script | derived |
| P-174 | Passenger coaches on the `IX` list, EMU/MEMU: 3-14, median 9 (15 of 22 have a list; records from 2017-2021). | IX /trains/<number> for the 22 EMU/MEMU rows (P-118 to P-139); counted by script | derived |
| P-175 | Delhi EMUs run with 12 coaches, ten general and two ladies'. MEMU lengths are not given there. | WP-DSR | secondary |
| P-176 | Mail/Express standard composition: 22 coaches, 12 general and sleeper non-AC plus 8 AC (the remaining 2 are not named). Seen in search-result excerpts of Lok Sabha unstarred Q.3745 (Dec 2024) and Q.2813 (6 Aug 2025). The PDFs were unreachable (Sources table, `LS-REPLIES`), so this is not read. | WebSearch excerpts of eparlib.sansad.in AU3745, AU2813 | unconfirmed |
| P-177 | A Rajdhani of 22 vehicles on LHB is 22 × 24.0 = 528 m of coaches, plus about 20.6 m of locomotive. | derived from the LHB-vehicle row, the Rajdhani notice row and the WAP-7 row | derived |
| P-178 | Coach count of a Rajdhani, Duronto, Shatabdi, ordinary Mail/Express and MEMU rake as an issued Railway Board or RDSO composition document. | looked for: indianrailways.gov.in/railwayboard (serves "Site Under Construction"), rdso.indianrailways.gov.in search, Lok Sabha/Rajya Sabha PDFs | UNSOURCED |
| P-179 | Length of one EMU/MEMU car in Indian Railways service. No page I read gives it. | looked for: WP-DSR, WP "Electric multiple unit", RDSO spec above | UNSOURCED |
| P-180 | An 8-coach Vande Bharat working at NDLS (the brief asked for 8 and 16). None of the six VB trains in the pool has 8 cars. | looked for: WP-VB service table, RY pages for the six VB numbers | UNSOURCED |

## D. Dwell at NDLS (question 3)

Two different quantities hide under "dwell". A **through train** has a scheduled halt printed on its page. A **terminating or starting train** has no halt; the only number available is the gap between a train arriving and its paired number leaving, and that gap is mostly time in a pit line or yard, not time at the platform.

| ID | Claim | Source | Conf |
|---|---|---|---|
| P-181 | Scheduled halt at NDLS, express through trains (Duronto 12259/12260, Poorva 12303/12304, Gorakhdham 12555): 15 to 25 min (900 to 1500 s), n = 5 trains (12259, 12260, 12303, 12304, 12555). | RY pages, halt field = std - sta at NDLS | derived |
| P-182 | Scheduled halt at NDLS, ordinary passenger (via Old Delhi or Nizamuddin; records from 2019): 2 to 10 min (120 to 600 s), n = 6 trains (51901, 51902, 54473, 54474, 54539, 54540). | RY pages, halt field = std - sta at NDLS | derived |
| P-183 | Scheduled halt at NDLS, EMU/MEMU through (records from 2018-2021): 2 to 10 min (120 to 600 s), n = 8 trains (04471, 04472, 64062, 64064, 64089, 64471, 64472, 64534). | RY pages, halt field = std - sta at NDLS | derived |
| P-184 | The halt for 64534 (Panipat-Ghaziabad MEMU) is 10 min on `RY` (19:55-20:05) but 2 min on `IX` (19:50-19:52); the halt for 54473 is 7 min on `RY` and 5 min on `IX`. Two aggregators disagree, so the passenger and MEMU halts are `unconfirmed` even as ranges. | RY /time-table/64534, /54473; IX /trains/64534, /54473 | unconfirmed |
| P-185 | No Rajdhani, Shatabdi, Vande Bharat, Humsafar, Tejas or Garib Rath in the pool is a through train at NDLS; all 68 of them (Rajdhani 26, Shatabdi 24, Jan Shatabdi 4, Vande Bharat 6, Humsafar 4, Tejas 2, Garib Rath 2) start or end here. | derived: `pos` of every pool row in section A | derived |
| P-186 | Gap from a terminating train's arrival to its paired number's departure, Rajdhani: 285 min (4h45) to 885 min (14h45), n = 13 pairs (shortest 12441→12442, longest 12426→12425; arriving→departing). **Not platform occupancy**; the same rake is not shown to work both trains. | RY pages, arr of the even/odd partner to dep of the other, mod 24 h | derived |
| P-187 | Gap from a terminating train's arrival to its paired number's departure, Shatabdi: 235 min (3h55) to 590 min (9h50), n = 12 pairs (shortest 12046→12045, longest 12012→12011; arriving→departing). **Not platform occupancy**; the same rake is not shown to work both trains. | RY pages, arr of the even/odd partner to dep of the other, mod 24 h | derived |
| P-188 | Gap from a terminating train's arrival to its paired number's departure, Jan Shatabdi: 170 min (2h50) to 255 min (4h15), n = 2 pairs (shortest 12058→12057, longest 12056→12055; arriving→departing). **Not platform occupancy**; the same rake is not shown to work both trains. | RY pages, arr of the even/odd partner to dep of the other, mod 24 h | derived |
| P-189 | Gap from a terminating train's arrival to its paired number's departure, Duronto: 340 min (5h40), n = 2 pairs (shortest 12273→12274, longest 12281→12282; arriving→departing). **Not platform occupancy**; the same rake is not shown to work both trains. | RY pages, arr of the even/odd partner to dep of the other, mod 24 h | derived |
| P-190 | Gap from a terminating train's arrival to its paired number's departure, Vande Bharat: 410 min (6h50) to 685 min (11h25), n = 3 pairs (shortest 22440→22439, longest 22448→22447; arriving→departing). **Not platform occupancy**; the same rake is not shown to work both trains. | RY pages, arr of the even/odd partner to dep of the other, mod 24 h | derived |
| P-191 | Gap from a terminating train's arrival to its paired number's departure, Humsafar: 655 min (10h55) to 990 min (16h30), n = 2 pairs (shortest 12349→12350, longest 12275→12276; arriving→departing). **Not platform occupancy**; the same rake is not shown to work both trains. | RY pages, arr of the even/odd partner to dep of the other, mod 24 h | derived |
| P-192 | Gap from a terminating train's arrival to its paired number's departure, Tejas: 175 min (2h55), n = 1 pair (shortest 82501→82502, longest 82501→82502; arriving→departing). **Not platform occupancy**; the same rake is not shown to work both trains. | RY pages, arr of the even/odd partner to dep of the other, mod 24 h | derived |
| P-193 | Gap from a terminating train's arrival to its paired number's departure, Garib Rath: 300 min (5h00), n = 1 pair (shortest 12877→12878, longest 12877→12878; arriving→departing). **Not platform occupancy**; the same rake is not shown to work both trains. | RY pages, arr of the even/odd partner to dep of the other, mod 24 h | derived |
| P-194 | Gap from a terminating train's arrival to its paired number's departure, Superfast: 330 min (5h30) to 1315 min (21h55), n = 15 pairs (shortest 12561→12562, longest 12419→12420; arriving→departing). **Not platform occupancy**; the same rake is not shown to work both trains. | RY pages, arr of the even/odd partner to dep of the other, mod 24 h | derived |
| P-195 | Gap from a terminating train's arrival to its paired number's departure, Mail/Express: 460 min (7h40) to 480 min (8h00), n = 2 pairs (shortest 14211→14212, longest 12723→12724; arriving→departing). **Not platform occupancy**; the same rake is not shown to work both trains. | RY pages, arr of the even/odd partner to dep of the other, mod 24 h | derived |
| P-196 | Gap from a terminating train's arrival to its paired number's departure, EMU/MEMU: 10 min (0h10) to 510 min (8h30), n = 3 pairs (shortest 64912→64911, longest 64449→64450; arriving→departing). **Not platform occupancy**; the same rake is not shown to work both trains. | RY pages, arr of the even/odd partner to dep of the other, mod 24 h | derived |
| P-197 | Rule for the other end of a Mail/Express run: "a minimum of 3 hours halt (except for intercity day trains) should be mandatorily provided for carrying out the cleaning and attention to passenger amenities besides continuity/bpc" (Board letter 2009/M(c)/141/2 of 12.09.2016, repeated in the 14.06.2017 maintenance pattern). Rajdhani/Duronto get under-gear and external cleaning at both ends. Shatabdi: internal cleaning at both ends, nothing about a minimum halt. | IRIMEE-MAINT, categories 1, 1A, 3(a)(i) | secondary |
| P-198 | This 3 h is time at the maintenance end, not the platform. Of the 56 pairs above, 3 have a gap under 180 min: 64912/64911 (10), 12058/12057 (170) and 82501/82502 (175). Jan Shatabdi and Tejas are day trains and the Rohtak MEMU is not a Mail/Express, so none contradicts the rule; the shortest Rajdhani or Shatabdi gap is 285 and 235 min. | derived from the gap rows and the rule row | derived |
| P-199 | How long a terminating or starting train occupies its NDLS platform before it moves to the yard, or after it arrives from it (platform occupancy, minutes). | looked for: RY, IX, IRIMEE-MAINT, WP-NDLS; no source gives it | UNSOURCED |

## E. Platforms (question 4)

| ID | Claim | Source | Conf |
|---|---|---|---|
| P-200 | New Delhi has 16 platforms; platform 1 is on the Paharganj side and platform 16 on the Ajmeri Gate side. The station handles over 250 passenger trains a day. | WP-NDLS | secondary |
| P-201 | Platform field on `RY` across the 139 pool pages: all 16 platforms appear. Count of trains per platform: 1:18, 2:8, 3:7, 4:4, 5:10, 6:10, 7:5, 8:5, 9:7, 10:6, 11:11, 12:10, 13:7, 14:15, 15:11, 16:5. | RY pages, counted by script | derived |
| P-202 | The same `RY` field is 0 (unknown) for 24,688 of 27,575 stop records across the 176 `RY` pages that carried a schedule (197 fetched), so it carries real data where it is non-zero. Its origin (historic running, allotment or editor entry) is not stated, so each PF in section A is `unconfirmed` as an allotment. | RY pages, counted by script | derived |
| P-203 | Some PF values repeat for a train and its pair (12301/12302/12305/12306 all on PF 14), others differ (12309 on PF 7, its pair 12310 on PF 9). A game can use a per-train "usual" platform but must not treat it as a roster. | RY pages 12301, 12305, 12309, 12310 | derived |
| P-204 | A published platform allotment for NDLS (Northern Railway / Delhi Division order, or NTES platform board). | looked for: WebSearch "NDLS platform allotment", totaltraininfo, railmitra, ixigo station pages, indiarailinfo station page (175 bytes), NTES (serves a "technical activity" redirect) | UNSOURCED |

## F. Locomotives (question 5)

| ID | Claim | Source | Conf |
|---|---|---|---|
| P-205 | WAP-7: length over beams 20,562 mm; loco weight 123 t. | WP-WAP7 infobox | secondary |
| P-206 | WAG-9: length 20.562 m; WAP-5: length over buffers 18,162 mm; WAP-4: length 18.794 m. WAP-7 and WAG-9 give the same figure. | WP-WAG9, WP-WAP5, WP-WAP4 infoboxes | secondary |
| P-207 | Caption: "12273 Howrah-New Delhi Duronto Express led by WAP 5 standing on Platform 5 at New Delhi Railway Station". | WP-DUR image caption | secondary |
| P-208 | Which locomotive class hauls each pool train, and the length of an RDSO/CLW type drawing. Vande Bharat and EMU/MEMU have no locomotive. | looked for: RY, IX, WP type articles; no per-train loco is given | UNSOURCED |

## Proposed starting values

Lengths are metres of rake including generator cars where a source shows them, **excluding** the locomotive (add 20.6 m for a loco-hauled train: P-205). "Solid" means a primary source and a count; "assumed" means a rule of mine on top of secondary data.

- Length, Vande Bharat 16-car: **384 m**. Count is solid (P-159, primary); length is secondary, at 24 m a car (P-158, P-160).
- Length, Vande Bharat 20-car (4 of the 6 VB trains at NDLS): **480 m**. P-158, P-160. Solid on count; the 8-car 192 m figure is secondary and no 8-car train reaches NDLS (P-180).
- Length, Rajdhani: **528 m** (22 LHB vehicles × 24.0 m). Assumed: 20 passenger coaches are counted (P-163, P-162) and the 2 generator cars come from one other Rajdhani's notice (P-161); 24.0 m is solid (P-156).
- Length, Shatabdi: **384 m** (16 chair/executive coaches × 24.0 m). Assumed: the source lists no generator cars and does not say LHB for every Shatabdi (P-164, P-156); the pool spans 11-18 coaches, so 264-432 m.
- Length, Duronto: **456 m** (19 × 24.0 m, median of 18-21). Assumed: coach type and power cars unknown (P-166).
- Length, Superfast and ordinary Mail/Express: **490-530 m** (22 vehicles at 22.3 m ICF to 24.0 m LHB). Assumed: 20 listed coaches (P-171, P-172) plus 2 end cars; 22 is the stated standard but unconfirmed (P-176); ICF/LHB per train is not in any source (P-156, P-157).
- Length, Humsafar 480 m, Garib Rath 432 m, Tejas (LKO-NDLS) 336 m, Jan Shatabdi 336 m: coaches × 24.0 m. Assumed, from P-168, P-170, P-169 and P-165; the Tejas count conflicts with Wikipedia's other Tejas.
- Length, Delhi EMU: **12 coaches ≈ 264 m** at an assumed 22 m a car. 12 coaches is secondary (P-175); car length is unsourced (P-179). MEMU rakes of 8-9 coaches ≈ 176-198 m (P-174), same assumption.
- Dwell, through express (Duronto, Poorva SF, Gorakhdham): **900-1500 s**, n = 5 trains (P-181). Solid as a schedule, thin as a sample.
- Dwell, through passenger: **120-600 s**, n = 6 (P-182); through EMU/MEMU: **120-600 s**, n = 8 (P-183). Assumed: 2017-2021 records, two aggregators disagree on two of them.
- Dwell, terminating or starting train at its platform: **no sourced value** (P-199). Starting guess 600-1200 s before the rake leaves for the yard, **assumed**. The sourced figures are rake gaps of 170-885 min for locomotive-hauled trains and a 3 h maintenance halt at the other end (P-197, P-186): they bound the rake, not the platform.
- Platforms: draw 1-16 (P-200) with the per-train PF of section A as a weighted default (P-201); no allotment source (P-204). Assumed.
- Locomotive length: **20.6 m** (WAP-7/WAG-9), 18.2 m (WAP-5). Secondary (P-205, P-206); which class hauls which train is unsourced (P-208).
- Pool size: **139 trains** by type: Rajdhani 26 (P-001 to P-026); Shatabdi 24 (P-027 to P-050); Jan Shatabdi 4 (P-051 to P-054); Duronto 6 (P-055 to P-060); Vande Bharat 6 (P-061 to P-066); Humsafar 4 (P-067 to P-070); Tejas 2 (P-071 to P-072); Garib Rath 2 (P-073 to P-074); Superfast 32 (P-075 to P-106); Mail/Express 5 (P-107 to P-111); Passenger 6 (P-112 to P-117); EMU/MEMU 22 (P-118 to P-139). Solid on existence in the `RY` records; the 28 trains with 2017-2021 records (Passenger 6, EMU/MEMU 22) are assumed still running. Cross-checks: P-150, P-142.
