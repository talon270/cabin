Written 2026-11-19 by Codex (`codex --search exec -s read-only`), at Claude's request, for `PLAN-redesign.md`. Claude has not opened the cited URLs. Overpass was unreachable from Codex's sandbox (DNS) and, the same day, from this machine (3 endpoints timed out after 30 to 40 s): every count below is reported or estimated, none measured.

# Candidate ladder: small stations to NDLS

Shell networking failed: DNS could not resolve `overpass-api.de`, so I could not measure Overpass counts. The one attempted request returned **0 response bytes** and no data. Station platform and track counts below are **reported by the linked sources**, not measured from Overpass; switch counts for the smaller stations are **rough estimates, unsourced**. The bounding boxes are proposed extraction boxes, not Overpass-verified extents.

| Rank | Station | Code | Bbox (south, west, north, east) | Platforms | Switches | Tracks | Skill taught | Extractor risk | Source |
|---:|---|---|---|---:|---:|---:|---|---|---|
| 1 | Holambi Kalan | HUK | `28.7940, 77.0940, 28.8085, 77.1110` | 2 (reported) | ≈2–4 (est., unsourced) | ≈3 (est., unsourced) | Work a simple station: receive a train into a platform loop, then send it back out. | A small box may cut the approach lines too close to the loop, creating portals inside the useful throat. Confirm each platform outline is tagged `railway=platform` and has a track within the extractor’s 6 m reach. | [Station code, zone, platforms](https://disha.rpunia.com/station/HUK); [OSM map](https://www.openstreetmap.org/#map=16/28.8012/77.1023) |
| 2 | Narela | NUR | `28.8375, 77.0760, 28.8560, 77.0970` | 2 (reported) | ≈4–8 (est., unsourced) | 5 (reported) | Work opposing trains on a double line and learn to route through crossovers. | Its reported five tracks make correct OSM connectivity and track classification important. If crossovers are tagged as yard or siding, the extractor may promote them to running track to reach platforms; verify this matches the intended lesson. | [Station code, zone, platforms](https://disha.rpunia.com/station/NUR); [reported tracks and location](https://en.wikipedia.org/wiki/Narela_railway_station); [OSM map](https://www.openstreetmap.org/#map=16/28.8465/77.0855) |
| 3 | Sahibabad Junction | SBB | `28.6660, 77.3340, 28.6860, 77.3570` | 5 (reported) | ≈8–14 (est., unsourced) | 8 (reported) | Choose between diverging routes at a junction while managing platform traffic. | The split toward Delhi and New Delhi may fall outside a tight box or be mistaken for ordinary running-line connectivity. A wider box helps retain the junction, but can add unrelated track and platforms. | [Station code, zone, platforms](https://railtribe.in/station/sbb); [junction description and reported tracks](https://en.wikipedia.org/wiki/Sahibabad_Junction_railway_station); [OSM map](https://www.openstreetmap.org/#map=15/28.6760/77.3455) |
| 4 | Delhi Sarai Rohilla | DEE | `28.6510, 77.1760, 28.6750, 77.1970` | 5 (reported) | ≈10–18 (est., unsourced) | 12 (reported) | Handle a terminal’s platform throat and the different route choices for trains arriving and departing. | This box may include nearby lines or stations as well as the terminal. Adjacent platform outlines can attach to the wrong track if they are closer than the intended track; inspect the extractor’s platform assignments before accepting an extract. | [Station code, zone, platforms](https://www.redbus.in/trains/stations/delhi-sarai-rohilla-dee); [lines and reported tracks](https://en.wikipedia.org/wiki/Delhi_Sarai_Rohilla_railway_station); [OSM map](https://www.openstreetmap.org/#map=15/28.6631/77.1864) |
| 5 | New Delhi | NDLS | `28.6320, 77.2100, 28.6520, 77.2280` | 16 (repo-reported) | 148 points (user-provided); 79 `railway=switch` nodes in the repo’s older plan | 20 (reported) | Combine the earlier skills across a dense, multi-platform station. | The repo already documents adjacent stations, duplicated platform outlines, assumed running-track promotions and many unreachable portal/platform pairs. Keep the NDLS overrides with this station; they refer to NDLS-specific OSM element IDs. | [Repo README](https://github.com/talon270/cabin/blob/main/README.md); [repo’s earlier OSM counts](https://github.com/talon270/cabin/blob/main/PLAN-cabin.md); [OSM map](https://www.openstreetmap.org/#map=15/28.6428/77.2191) |

**Recommended first station: Holambi Kalan.** Its reported two platforms make it the best candidate for teaching the basic receive-and-depart cycle with minimal route choice. It is also a short step from the intended Delhi-area Northern Railway setting. That makes it a reasonable first prototype, subject to checking the OSM geometry and extractor reachability.

## Overpass attempt and extractor notes

I attempted this count query for a trial bounding box:

```sh
curl -sS --fail --max-time 20 -A 'cabin-research/1.0' \
  --data-urlencode 'data=[out:json][timeout:15];(node(28.722,77.101,28.744,77.121)["railway"="switch"];way(28.722,77.101,28.744,77.121)["railway"="platform"];way(28.722,77.101,28.744,77.121)["railway"="rail"]);out count;' \
  https://overpass-api.de/api/interpreter -o /tmp/cabin-overpass-test.json
```

**Output:** `curl: (6) Could not resolve host: overpass-api.de`. Response body: **0 bytes**. That box was only a connectivity trial; no station counts were obtained.

The extractor’s main assumptions and likely failure modes are visible in [`crates/osm/src/main.rs`](https://github.com/talon270/cabin/blob/main/crates/osm/src/main.rs), [`fetch-osm.sh`](https://github.com/talon270/cabin/blob/main/fetch-osm.sh), and [`data/ndls.overrides.toml`](https://github.com/talon270/cabin/blob/main/data/ndls.overrides.toml):

- It treats running rail ways cut by the box edge as portals. A box that is too tight can therefore turn a station throat into an artificial entry or exit.
- It recognizes platforms from `railway=platform` ways and attaches them to nearby track by proximity. If a platform is missing that tag, or the nearest track is wrong, it can be omitted or misattached. The `[platform]` override pins a platform way to its intended track way.
- It infers points and crossings from shared track nodes and their degree; geometry determines the point’s stem and legs. Disconnected OSM nodes, ambiguous geometry or unusual slips can produce the wrong topology. `[join]` repairs a missing connection.
- It reads `service=crossover` as running track, but may also promote yard or siding ways when needed for platform reachability. `[kind]` can correct a way’s classification, and `[drop]` can exclude a way.
- Signal overrides move or rename generated signals; they do not repair track topology. The NDLS override file also has a 20-line limit documented by the extractor, so a ladder station should get its own overrides rather than inherit NDLS element IDs.

The skills in the table are suggested teaching goals inferred from the reported station size and layouts. The smaller-station switch counts are deliberately estimates: they need an Overpass fetch or direct OSM inspection before being used as game data.

## Measured 2026-11-19 (B0)

Fetched by you with `./fetch-osm.sh <code> <box>` on another network; extracted here with `cabin-osm <code>`. SBB and DEE: Overpass answered HTTP 502 on the first try; fetched on the second.

| Code | OSM file | Rail ways | Platforms | Points (graph) | `railway=switch` tags | Portals | Signals | Routes | Gate |
|---|---|---|---|---|---|---|---|---|---|
| HUK | 5,941 bytes | 6 (2 tagged siding, promoted: the platform loops) | 2, open lines, no `ref` | 6 | 0 | 4 | 8 | 12 | passed |
| NUR | 13,042 bytes | 9 (2 sidings) | 2, open lines, no `ref` | 10 | 0 | 4 | 8 | 12 | passed |
| SBB | 37,147 bytes | 36 running edges | **0: the box misses the station** (no platform, nothing named Sahibabad) | 12 | 11 | 15 | 15 | 36 | failed: "no platform found" |
| DEE | 86,726 bytes | 62 running, 40 siding edges (10 promoted) | 4 ways: 6 platforms (`ref=2, 3` and `4, 5` are islands) | 44 | 2 | 14 | 26 | 85 | passed |

Both first failed the gate: "platform ... has no track within 6 m", with tracks 3 to 4 m away. The extractor treated each open platform line as an area outline, so every track on one side counted as inside it, and a 2-node line never voted. Fixed in `crates/osm/src/main.rs`; the NDLS output is byte-identical (sha256).

DEE first stopped on "the yard did not survive a write and read back": its island platforms carry `ref=2, 3` with a comma, the extractor split refs on `;` only, and the name "2, 3" broke the yard file. Fixed (split on `;` and `,`). **DEE's box is too wide for a ladder rung:** 14 portals against NDLS's 12; a tighter box, measured on a map, should leave only the terminal's own lines. **SBB needs a box that holds the station:** Codex's coordinates are a guess, and one web search found none for the junction itself.
