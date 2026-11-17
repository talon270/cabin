//! CABIN-OSM · NDLS YARD EXTRACTOR
//!   · reads data/ndls.osm.json, the raw Overpass answer (fetch-osm.sh)
//!   · applies data/ndls.overrides.toml: fixes OpenStreetMap gets wrong
//!   · prints a data-quality report: junctions, points, dead ends, portals,
//!     platform-to-portal reachability
//!   · generates signals by rule (cabin-sim `signal.rs`), applies the
//!     overrides' [signal] lines, and reports the routes they give
//!   · runs the gate, then compares the new yard with the one on disk, and
//!     writes data/ndls.yard only when both pass
//!
//! Usage, from anywhere:  cargo run -p cabin-osm [-- --accept-worse]
//!
//! Runs by hand, once per refresh. The game never fetches. The output is
//! deterministic: ways and nodes are walked in OSM-id order, so the same raw
//! file always writes the same bytes, and a test below demands that the
//! committed `.yard` matches its sources exactly.
//!
//! The gate passes when every platform track can be reached from at least
//! one portal, and can leave by at least one, by a train that obeys the
//! points (arrive on a stem: choose a leg; arrive on a leg: take the stem),
//! over `Running` track only. It does not mean the yard is right; it means
//! the graph is not obviously broken.
//!
//! The comparison is Guard 2 of the plan: a bad extract never replaces a good
//! yard. A yard that loses a platform, a portal, a portal-to-platform path or
//! a track way that the yard on disk has is refused, because a truncated
//! Overpass answer or a deleted crossover still passes the gate.
//! `--accept-worse` writes it anyway, for a loss you meant, such as a new
//! [drop] line.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use cabin_sim::yard::*;
use serde_json::Value;

/// The box `fetch-osm.sh` asks for: south, west, north, east. The extract is
/// clipped to it, so a running line leaving it becomes a portal.
const BBOX: (f64, f64, f64, f64) = (28.632, 77.210, 28.652, 77.228);

/// Mean Earth radius (IUGG). An equirectangular projection around the box
/// centre is off by under 0.1% across 2 km, which no rule here feels.
const EARTH_R_M: f64 = 6_371_008.8;

/// A leg's direction is measured this far from the junction, not from the
/// first node: OSM places nodes a metre or two apart at points, and a
/// 1 m stub gives a noisy angle.
const DIR_SPAN_M: f64 = 12.0;

/// The stem of a point is the edge furthest from both others. Under this
/// angle to the nearest of them, the junction is a T or a sharp Y and the
/// stem is a guess; the report lists it.
const STEM_CONFIDENT_DEG: f64 = 120.0;

/// Normal and reverse legs whose deviation from straight differs by less than
/// this cannot be told apart by angle; the report lists them.
const LEG_AMBIGUOUS_DEG: f64 = 5.0;

/// Four tracks at one node tagged `railway=switch` form a slip, not a diamond:
/// a train may pass between two of them when they meet at least this close to
/// straight. Measured at NDLS on 2026-11-16, all 15 four-track nodes are
/// two tracks crossing at about 6 degrees: straight pairs 175 to 180 degrees,
/// diverging pairs 166 to 174, acute pairs 14 or less. Any value from 20 to
/// 165 sorts them the same way. A double slip, with all four diverging moves,
/// is assumed: OpenStreetMap does not say single or double.
const SLIP_THROUGH_DEG: f64 = 150.0;

/// A platform is attached to the tracks beside its outline: track samples
/// within `PLATFORM_REACH_M` of the outline, along its length, vote for the
/// side they lie on. A track centre stands a few metres from the platform
/// edge; 6 m does not reach the next island's track at about 5 m spacing
/// plus a platform width. Share is the part of the platform's length the
/// winning edge runs beside; under half, the track is split by points and the
/// longest piece was taken.
const PLATFORM_SAMPLE_M: f64 = 5.0;
const PLATFORM_REACH_M: f64 = 6.0;
const PLATFORM_CONFIDENT_SHARE: f64 = 0.5;

/// Plan, pre-mortem: if the extract needs more override lines than this,
/// stop and ask whether to hand-draw the yard.
const MAX_OVERRIDE_LINES: usize = 20;

type Tags = BTreeMap<String, String>;

struct Way {
    id: u64,
    nodes: Vec<u64>,
    tags: Tags,
}

struct Osm {
    base: String,
    nodes: HashMap<u64, (f64, f64, Tags)>,
    ways: Vec<Way>,
}

#[derive(Debug, Default)]
struct Overrides {
    /// way id -> kind, for a way OpenStreetMap tagged as the wrong service.
    kind: BTreeMap<u64, Kind>,
    /// platform way id -> the track way it serves, when proximity picks wrong.
    platform: BTreeMap<u64, u64>,
    /// ways to ignore.
    drop: BTreeSet<u64>,
    /// node id -> node id it is merged into, for a missing junction.
    join: BTreeMap<u64, u64>,
    /// generated signal name -> how to move or rename it.
    signal: BTreeMap<String, SignalOverride>,
}

/// One `[signal]` line: `HN4 = { shift_m = -40.0, name = "N4H" }`. Keyed by
/// the generated name, which is stable while the track is: a re-extract
/// that drops the signal refuses the build, as a missing way does.
#[derive(Debug, Default, Clone)]
struct SignalOverride {
    /// Metres to move it along its edge, positive in the direction it faces.
    shift_m: f64,
    name: Option<String>,
}

impl Overrides {
    fn lines(&self) -> usize {
        self.kind.len() + self.platform.len() + self.drop.len() + self.join.len() + self.signal.len()
    }
}

fn tags_of(v: &Value) -> Tags {
    let mut t = Tags::new();
    if let Some(o) = v.get("tags").and_then(|x| x.as_object()) {
        for (k, val) in o {
            if let Some(s) = val.as_str() {
                t.insert(k.clone(), s.to_string());
            }
        }
    }
    t
}

fn parse_osm(text: &str) -> Result<Osm, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("osm json: {e}"))?;
    let base = v["osm3s"]["timestamp_osm_base"].as_str().unwrap_or("unknown").to_string();
    let mut nodes = HashMap::new();
    let mut ways = Vec::new();
    for el in v["elements"].as_array().ok_or("osm json: no elements")? {
        let id = el["id"].as_u64().ok_or("element without id")?;
        match el["type"].as_str() {
            Some("node") => {
                let (Some(lat), Some(lon)) = (el["lat"].as_f64(), el["lon"].as_f64()) else {
                    continue;
                };
                nodes.insert(id, (lat, lon, tags_of(el)));
            }
            Some("way") => {
                let ns = el["nodes"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|n| n.as_u64()).collect())
                    .unwrap_or_default();
                ways.push(Way { id, nodes: ns, tags: tags_of(el) });
            }
            _ => {}
        }
    }
    ways.sort_by_key(|w| w.id);
    Ok(Osm { base, nodes, ways })
}

fn parse_overrides(text: &str) -> Result<Overrides, String> {
    let t: toml::Table = text.parse().map_err(|e| format!("overrides.toml: {e}"))?;
    let mut o = Overrides::default();
    let id = |k: &str| k.parse::<u64>().map_err(|_| format!("overrides.toml: '{k}' is not an OSM id"));
    for (section, body) in &t {
        let table = body.as_table().ok_or(format!("overrides.toml: [{section}] must be a table"))?;
        for (k, val) in table {
            match section.as_str() {
                "kind" => {
                    let name = val.as_str().unwrap_or("");
                    let kind = Kind::from_name(name)
                        .ok_or(format!("overrides.toml: [kind] {k}: '{name}' is not running, yard or siding"))?;
                    o.kind.insert(id(k)?, kind);
                }
                "platform" => {
                    let w = val.as_integer().ok_or(format!("overrides.toml: [platform] {k} needs a way id"))?;
                    o.platform.insert(id(k)?, w as u64);
                }
                "drop" => {
                    // `= false` reads as "keep it"; dropping anyway would be a
                    // silent surprise, so only `true` is accepted.
                    if val.as_bool() != Some(true) {
                        return Err(format!("overrides.toml: [drop] {k} must be true; delete the line to keep the way"));
                    }
                    o.drop.insert(id(k)?);
                }
                "join" => {
                    let n = val.as_integer().ok_or(format!("overrides.toml: [join] {k} needs a node id"))?;
                    o.join.insert(id(k)?, n as u64);
                }
                "signal" => {
                    let bad = |what: &str| format!("overrides.toml: [signal] {k}: {what}");
                    let t = val.as_table().ok_or(bad("needs { shift_m = ..., name = ... }"))?;
                    let mut so = SignalOverride::default();
                    for (field, v) in t {
                        match field.as_str() {
                            "shift_m" => so.shift_m = v.as_float().or(v.as_integer().map(|i| i as f64)).ok_or(bad("shift_m must be a number"))?,
                            "name" => so.name = Some(v.as_str().filter(|n| !n.is_empty() && !n.contains(char::is_whitespace)).ok_or(bad("name must be one word"))?.to_string()),
                            other => return Err(bad(&format!("unknown field '{other}'"))),
                        }
                    }
                    o.signal.insert(k.clone(), so);
                }
                other => return Err(format!("overrides.toml: unknown section [{other}]")),
            }
        }
    }
    Ok(o)
}

// ---- geometry ---------------------------------------------------------------

type P = (f64, f64);

fn sub(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}

fn len(a: P) -> f64 {
    (a.0 * a.0 + a.1 * a.1).sqrt()
}

fn poly_len(poly: &[P]) -> f64 {
    poly.windows(2).map(|w| len(sub(w[1], w[0]))).sum()
}

fn seg_dist(p: P, a: P, b: P) -> f64 {
    let ab = sub(b, a);
    let l2 = ab.0 * ab.0 + ab.1 * ab.1;
    let t = if l2 == 0.0 { 0.0 } else { ((p.0 - a.0) * ab.0 + (p.1 - a.1) * ab.1) / l2 }.clamp(0.0, 1.0);
    len(sub(p, (a.0 + t * ab.0, a.1 + t * ab.1)))
}

fn poly_dist(p: P, poly: &[P]) -> f64 {
    poly.windows(2).map(|w| seg_dist(p, w[0], w[1])).fold(f64::INFINITY, f64::min)
}

/// Points along a polyline every `step` metres, from its first point.
fn samples(poly: &[P], step: f64) -> Vec<P> {
    let mut out = vec![poly[0]];
    let mut carry = 0.0;
    for w in poly.windows(2) {
        let seg = len(sub(w[1], w[0]));
        let mut d = step - carry;
        while d <= seg {
            let t = d / seg;
            out.push((w[0].0 + t * (w[1].0 - w[0].0), w[0].1 + t * (w[1].1 - w[0].1)));
            d += step;
        }
        carry = seg - (d - step);
    }
    out
}

/// Unit vector from the end of an edge towards the point `DIR_SPAN_M` along it.
fn dir_unit(poly: &[P], at_start: bool) -> P {
    let pts: Vec<P> = if at_start { poly.to_vec() } else { poly.iter().rev().copied().collect() };
    let mut run = 0.0;
    let mut target = pts[pts.len() - 1];
    for w in pts.windows(2) {
        let seg = len(sub(w[1], w[0]));
        if run + seg >= DIR_SPAN_M {
            let t = (DIR_SPAN_M - run) / seg;
            target = (w[0].0 + t * (w[1].0 - w[0].0), w[0].1 + t * (w[1].1 - w[0].1));
            break;
        }
        run += seg;
    }
    let v = sub(target, pts[0]);
    let l = len(v);
    if l == 0.0 { (1.0, 0.0) } else { (v.0 / l, v.1 / l) }
}

fn angle_deg(u: P, v: P) -> f64 {
    (u.0 * v.0 + u.1 * v.1).clamp(-1.0, 1.0).acos().to_degrees()
}

// ---- graph ------------------------------------------------------------------

/// What a graph node is, from the edges that meet at it.
#[derive(Clone, Debug)]
enum Class {
    Dead,
    Through,
    Point { stem: usize, normal: usize, reverse: usize },
    Cross { pairs: Vec<(usize, usize)> },
    Complex,
}

struct Frag {
    way: u64,
    kind: Kind,
    nodes: Vec<u64>,
    cut_start: bool,
    cut_end: bool,
}

#[derive(Default)]
struct Report {
    lines: Vec<String>,
    /// Gate failures, one per platform that some portal cannot reach.
    failures: Vec<String>,
}

impl Report {
    fn say(&mut self, s: impl Into<String>) {
        self.lines.push(s.into());
    }
}

fn kind_of(tags: &Tags) -> Option<Kind> {
    // Crossovers join running lines; a train takes them as part of a route, so
    // they are worked. Assumed, not read from a rule: the report counts them.
    if tags.get("usage").map(|s| s.as_str()) == Some("main") {
        return Some(Kind::Running);
    }
    match tags.get("service").map(|s| s.as_str()) {
        Some("crossover") => Some(Kind::Running),
        Some("yard") => Some(Kind::Yard),
        Some("siding") => Some(Kind::Siding),
        None => Some(Kind::Running),
        Some(_) => None,
    }
}

/// One step of a train: from (edge, forward) the states it can enter next.
fn moves(
    yard: &Yard,
    class: &[Class],
    inc: &[Vec<(usize, bool)>],
    e: usize,
    fwd: bool,
    allowed: &dyn Fn(Kind) -> bool,
) -> Vec<(usize, bool)> {
    let n = if fwd { yard.edges[e].b } else { yard.edges[e].a };
    let next: Vec<usize> = match &class[n] {
        Class::Dead => vec![],
        Class::Through => inc[n].iter().map(|x| x.0).filter(|&x| x != e).collect(),
        Class::Point { stem, normal, reverse } => {
            if e == *stem { vec![*normal, *reverse] } else { vec![*stem] }
        }
        Class::Cross { pairs } => pairs
            .iter()
            .filter_map(|&(p, q)| if p == e { Some(q) } else if q == e { Some(p) } else { None })
            .collect(),
        Class::Complex => inc[n].iter().map(|x| x.0).filter(|&x| x != e).collect(),
    };
    next.into_iter()
        .filter(|&x| allowed(yard.edges[x].kind))
        .map(|x| (x, yard.edges[x].a == n))
        .collect()
}

/// Edges a train can run over from `start`, and the graph nodes it can arrive at.
fn reach(
    yard: &Yard,
    class: &[Class],
    inc: &[Vec<(usize, bool)>],
    start: (usize, bool),
    allowed: &dyn Fn(Kind) -> bool,
) -> (BTreeSet<usize>, BTreeSet<usize>) {
    let mut seen: BTreeSet<(usize, bool)> = BTreeSet::new();
    let mut q = VecDeque::new();
    let mut arrived = BTreeSet::new();
    seen.insert(start);
    q.push_back(start);
    while let Some((e, fwd)) = q.pop_front() {
        arrived.insert(if fwd { yard.edges[e].b } else { yard.edges[e].a });
        for s in moves(yard, class, inc, e, fwd, allowed) {
            if seen.insert(s) {
                q.push_back(s);
            }
        }
    }
    (seen.iter().map(|s| s.0).collect(), arrived)
}

/// The fewest not-yet-running ways a train from `start` must use to reach the
/// `target` edge, found by 0-1 search: entering a running track costs 0,
/// entering a new yard or siding way costs 1. `None` means no path exists over
/// any track, whatever the tags say. The result is the ways to promote, not a
/// proof of minimum: ways are counted where the path changes way.
fn cheapest_path(
    yard: &Yard,
    class: &[Class],
    inc: &[Vec<(usize, bool)>],
    start: (usize, bool),
    target: usize,
) -> Option<BTreeSet<u64>> {
    let free = |e: usize| yard.edges[e].kind == Kind::Running;
    let mut dist: HashMap<(usize, bool), u32> = HashMap::new();
    let mut parent: HashMap<(usize, bool), (usize, bool)> = HashMap::new();
    let mut q = VecDeque::new();
    dist.insert(start, 0);
    q.push_back(start);
    let mut end: Option<(usize, bool)> = None;
    while let Some(cur) = q.pop_front() {
        let d = dist[&cur];
        if cur.0 == target && end.is_none_or(|e| d < dist[&e]) {
            end = Some(cur);
        }
        for nx in moves(yard, class, inc, cur.0, cur.1, &|_| true) {
            let cost = if free(nx.0) || yard.edges[nx.0].way == yard.edges[cur.0].way { 0 } else { 1 };
            let nd = d + cost;
            if dist.get(&nx).is_none_or(|&old| nd < old) {
                dist.insert(nx, nd);
                parent.insert(nx, cur);
                if cost == 0 { q.push_front(nx) } else { q.push_back(nx) }
            }
        }
    }
    let mut cur = end?;
    let mut ways = BTreeSet::new();
    loop {
        if !free(cur.0) {
            ways.insert(yard.edges[cur.0].way);
        }
        match parent.get(&cur) {
            Some(&p) => cur = p,
            None => break,
        }
    }
    Some(ways)
}

fn kinds_summary(yard: &Yard) -> String {
    let mut by_kind: BTreeMap<&str, (usize, f64)> = BTreeMap::new();
    for ed in &yard.edges {
        let v = by_kind.entry(ed.kind.name()).or_default();
        v.0 += 1;
        v.1 += ed.len;
    }
    by_kind.iter().map(|(k, (c, l))| format!("{k} {c} edges/{l:.0} m")).collect::<Vec<_>>().join(", ")
}

fn extract(osm: &Osm, ov: &Overrides) -> Result<(Yard, Report), String> {
    let mut rep = Report::default();
    let (s, w, n, e) = BBOX;
    let (lat0, lon0) = ((s + n) / 2.0, (w + e) / 2.0);
    let cos0 = lat0.to_radians().cos();
    let project = |lat: f64, lon: f64| -> P {
        (
            (lon - lon0).to_radians() * EARTH_R_M * cos0,
            (lat - lat0).to_radians() * EARTH_R_M,
        )
    };
    let inside = |id: u64| -> bool {
        osm.nodes.get(&id).is_some_and(|&(lat, lon, _)| lat >= s && lat <= n && lon >= w && lon <= e)
    };
    let xy = |id: u64| -> Option<P> { osm.nodes.get(&id).map(|&(lat, lon, _)| project(lat, lon)) };

    // Overrides naming something OpenStreetMap no longer holds refuse the build.
    let way_ids: BTreeSet<u64> = osm.ways.iter().map(|w| w.id).collect();
    let mut missing = Vec::new();
    for k in ov.kind.keys().chain(ov.platform.keys()).chain(ov.drop.iter()) {
        if !way_ids.contains(k) {
            missing.push(format!("way {k}"));
        }
    }
    for t in ov.platform.values() {
        if !way_ids.contains(t) {
            missing.push(format!("way {t}"));
        }
    }
    for (a, b) in &ov.join {
        for k in [a, b] {
            if !osm.nodes.contains_key(k) {
                missing.push(format!("node {k}"));
            }
        }
    }
    if !missing.is_empty() {
        return Err(format!("overrides name elements OpenStreetMap no longer has: {}", missing.join(", ")));
    }
    let merged = |id: u64| *ov.join.get(&id).unwrap_or(&id);

    // 1. Track ways -> fragments inside the box.
    let mut frags: Vec<Frag> = Vec::new();
    let (mut unclassified, mut outside_ways) = (0usize, 0usize);
    let mut way_kind: BTreeMap<u64, Kind> = BTreeMap::new();
    for way in &osm.ways {
        if way.tags.get("railway").map(|s| s.as_str()) != Some("rail") || ov.drop.contains(&way.id) {
            continue;
        }
        let kind = match ov.kind.get(&way.id) {
            Some(k) => *k,
            None => kind_of(&way.tags).unwrap_or_else(|| {
                unclassified += 1;
                Kind::Yard
            }),
        };
        way_kind.insert(way.id, kind);
        let mut run: Vec<u64> = Vec::new();
        let mut cut_start = false;
        let mut any = false;
        let flush = |run: &mut Vec<u64>, cs: bool, ce: bool, frags: &mut Vec<Frag>| {
            run.dedup();
            if run.len() >= 2 {
                frags.push(Frag { way: way.id, kind, nodes: std::mem::take(run), cut_start: cs, cut_end: ce });
            } else {
                run.clear();
            }
        };
        for (i, &nid) in way.nodes.iter().enumerate() {
            if inside(nid) {
                if run.is_empty() {
                    cut_start = i > 0;
                }
                run.push(merged(nid));
                any = true;
            } else if !run.is_empty() {
                flush(&mut run, cut_start, true, &mut frags);
            }
        }
        flush(&mut run, cut_start, false, &mut frags);
        if !any {
            outside_ways += 1;
        }
    }

    // 2. Junctions: a fragment's ends, and any node two fragments share.
    let mut uses: BTreeMap<u64, u32> = BTreeMap::new();
    for f in &frags {
        for (i, &nid) in f.nodes.iter().enumerate() {
            let end = i == 0 || i == f.nodes.len() - 1;
            *uses.entry(nid).or_default() += if end { 2 } else { 1 };
        }
    }
    let junction = |nid: u64| uses.get(&nid).copied().unwrap_or(0) >= 2;

    // 3. Edges between junctions.
    struct RawEdge {
        a: u64,
        b: u64,
        kind: Kind,
        way: u64,
        poly: Vec<P>,
    }
    let mut raw: Vec<RawEdge> = Vec::new();
    for f in &frags {
        let mut start = 0;
        for i in 1..f.nodes.len() {
            if junction(f.nodes[i]) {
                let poly: Vec<P> = f.nodes[start..=i].iter().filter_map(|&x| xy(x)).collect();
                if poly.len() >= 2 {
                    raw.push(RawEdge { a: f.nodes[start], b: f.nodes[i], kind: f.kind, way: f.way, poly });
                }
                start = i;
            }
        }
    }
    let node_ids: BTreeSet<u64> = raw.iter().flat_map(|r| [r.a, r.b]).collect();
    let index: BTreeMap<u64, usize> = node_ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
    let mut yard = Yard { origin: (lat0, lon0), osm_base: osm.base.clone(), ..Yard::default() };
    for &id in &node_ids {
        let p = xy(id).unwrap_or((0.0, 0.0));
        yard.nodes.push(Node { osm: id, x: p.0, y: p.1 });
    }
    for r in &raw {
        let l = poly_len(&r.poly);
        yard.edges.push(Edge { a: index[&r.a], b: index[&r.b], kind: r.kind, len: l, way: r.way, poly: r.poly.clone() });
    }
    let mut inc: Vec<Vec<(usize, bool)>> = vec![Vec::new(); yard.nodes.len()];
    for (i, ed) in yard.edges.iter().enumerate() {
        inc[ed.a].push((i, true));
        inc[ed.b].push((i, false));
    }

    // 4. Classify every graph node.
    let tag = |osm_id: u64, k: &str| -> Option<&str> {
        osm.nodes.get(&osm_id).and_then(|(_, _, t)| t.get(k)).map(|s| s.as_str())
    };
    let mut class: Vec<Class> = Vec::with_capacity(yard.nodes.len());
    let (mut unclear_stem, mut ambiguous_legs, mut untagged_dead, mut untagged_cross) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut complex = Vec::new();
    let mut slips: Vec<u64> = Vec::new();
    let mut buffers = 0usize;
    for (ni, list) in inc.iter().enumerate() {
        let oid = yard.nodes[ni].osm;
        let dirs: Vec<P> = list.iter().map(|&(e, st)| dir_unit(&yard.edges[e].poly, st)).collect();
        let c = match list.len() {
            0 | 1 => {
                if tag(oid, "railway") == Some("buffer_stop") {
                    buffers += 1;
                }
                Class::Dead
            }
            2 => Class::Through,
            3 => {
                // The stem is the edge furthest from both others.
                let score = |i: usize| -> f64 {
                    (0..3).filter(|&j| j != i).map(|j| angle_deg(dirs[i], dirs[j])).fold(f64::INFINITY, f64::min)
                };
                let stem_i = (0..3).max_by(|&a, &b| score(a).total_cmp(&score(b))).unwrap();
                if score(stem_i) < STEM_CONFIDENT_DEG {
                    unclear_stem.push((oid, score(stem_i)));
                }
                let legs: Vec<usize> = (0..3).filter(|&j| j != stem_i).collect();
                let dev = |j: usize| 180.0 - angle_deg(dirs[stem_i], dirs[j]);
                let (nl, rl) = if dev(legs[0]) <= dev(legs[1]) { (legs[0], legs[1]) } else { (legs[1], legs[0]) };
                if (dev(rl) - dev(nl)).abs() < LEG_AMBIGUOUS_DEG {
                    ambiguous_legs.push((oid, dev(nl), dev(rl)));
                }
                Class::Point { stem: list[stem_i].0, normal: list[nl].0, reverse: list[rl].0 }
            }
            4 => {
                let mut rest: Vec<usize> = vec![1, 2, 3];
                let opp = *rest.iter().max_by(|&&a, &&b| angle_deg(dirs[0], dirs[a]).total_cmp(&angle_deg(dirs[0], dirs[b]))).unwrap();
                rest.retain(|&x| x != opp);
                let mut pairs = vec![(list[0].0, list[opp].0), (list[rest[0]].0, list[rest[1]].0)];
                match tag(oid, "railway") {
                    Some("railway_crossing") => {}
                    Some("switch") => {
                        // The diverging pairs, after the straight ones.
                        slips.push(oid);
                        for i in 0..4 {
                            for j in i + 1..4 {
                                let p = (list[i].0, list[j].0);
                                let straight = pairs[..2].iter().any(|&q| q == p || q == (p.1, p.0));
                                if !straight && angle_deg(dirs[i], dirs[j]) >= SLIP_THROUGH_DEG {
                                    pairs.push(p);
                                }
                            }
                        }
                    }
                    _ => untagged_cross.push(oid),
                }
                Class::Cross { pairs }
            }
            _ => {
                complex.push((oid, list.len()));
                Class::Complex
            }
        };
        if matches!(c, Class::Dead) && tag(oid, "railway") != Some("buffer_stop") {
            untagged_dead.push(ni);
        }
        class.push(c);
    }
    for (ni, c) in class.iter().enumerate() {
        match c {
            Class::Point { stem, normal, reverse } => {
                yard.points.push(Point { node: ni, stem: *stem, normal: *normal, reverse: *reverse })
            }
            Class::Cross { pairs } => yard.crossings.push(Crossing { node: ni, pairs: pairs.clone() }),
            _ => {}
        }
    }

    // 5. Portals: a running fragment cut by the box edge.
    let mut portal_nodes: BTreeSet<usize> = BTreeSet::new();
    let mut other_exits = 0usize;
    for f in &frags {
        for (cut, nid) in [(f.cut_start, f.nodes[0]), (f.cut_end, f.nodes[f.nodes.len() - 1])] {
            if !cut {
                continue;
            }
            if f.kind == Kind::Running {
                if let Some(&ni) = index.get(&nid) {
                    portal_nodes.insert(ni);
                }
            } else {
                other_exits += 1;
            }
        }
    }
    let mut named: BTreeMap<String, Vec<(f64, usize)>> = BTreeMap::new();
    for &ni in &portal_nodes {
        let (x, y) = (yard.nodes[ni].x, yard.nodes[ni].y);
        let (side, along) = if y.abs() >= x.abs() { (if y > 0.0 { "N" } else { "S" }, x) } else { (if x > 0.0 { "E" } else { "W" }, y) };
        named.entry(side.to_string()).or_default().push((along, ni));
    }
    for (side, list) in named.iter_mut() {
        list.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (k, &(_, ni)) in list.iter().enumerate() {
            // One portal per running way at the node: N4 has two, and a train
            // must be able to enter by either. The extra ones take a letter.
            let ways = inc[ni].iter().filter(|&&(e, _)| yard.edges[e].kind == Kind::Running);
            for (j, &(edge, _)) in ways.enumerate() {
                let suffix = if j == 0 { String::new() } else { ((b'a' + j as u8) as char).to_string() };
                yard.portals.push(Portal { name: format!("{side}{}{suffix}", k + 1), node: ni, edge });
            }
        }
    }

    // A portal is an end for every search below, as `Yard::graph` makes it:
    // `class` was built from degree before portals were known, so N4, where
    // two running ways meet, would otherwise let a train turn back.
    for p in &yard.portals {
        if matches!(class[p.node], Class::Through) {
            class[p.node] = Class::Dead;
        }
    }

    // 6. Platforms. OpenStreetMap draws each platform as a closed outline, an
    // island serving two tracks ("6;7") or a single face ("16"), and does not
    // say which track is which number. Tracks beside the outline vote for the
    // side they lie on; numbers then run in ascending order across the yard,
    // which the outlines' positions confirm (platform 1 west, platform 16 east).
    // Assumed, not read from a rule: the report prints each attachment.
    struct Attach {
        name: String,
        way: u64,
        edge: Option<usize>,
        share: f64,
        len: f64,
    }
    let mut attaches: Vec<Attach> = Vec::new();
    let (mut platform_ways, mut skipped_platforms) = (0usize, 0usize);
    for pw in &osm.ways {
        if pw.tags.get("railway").map(|s| s.as_str()) != Some("platform") || ov.drop.contains(&pw.id) {
            continue;
        }
        // Metro platforms share the tag; they have no track in this graph.
        if pw.tags.get("subway").is_some() || pw.tags.get("location").map(|s| s.as_str()) == Some("underground") {
            skipped_platforms += 1;
            continue;
        }
        platform_ways += 1;
        let ring: Vec<P> = pw.nodes.iter().filter(|&&n| inside(n)).filter_map(|&n| xy(n)).collect();
        let mut refs: Vec<String> = pw
            .tags
            .get("ref")
            .map(|r| r.split(';').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
            .unwrap_or_default();
        refs.sort_by_key(|r| (r.parse::<u32>().unwrap_or(u32::MAX), r.clone()));
        if refs.is_empty() {
            refs.push(format!("w{}", pw.id));
        }
        if ring.len() < 3 {
            for r in refs {
                attaches.push(Attach { name: r, way: pw.id, edge: None, share: 0.0, len: 0.0 });
            }
            continue;
        }
        let nf = ring.len() as f64;
        let c = (ring.iter().map(|p| p.0).sum::<f64>() / nf, ring.iter().map(|p| p.1).sum::<f64>() / nf);
        let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
        for p in &ring {
            let d = sub(*p, c);
            sxx += d.0 * d.0;
            syy += d.1 * d.1;
            sxy += d.0 * d.1;
        }
        let th = 0.5 * (2.0 * sxy).atan2(sxx - syy);
        let u = (th.cos(), th.sin());
        // Lateral axis points east (or north when the outline runs east-west).
        let mut v = (-th.sin(), th.cos());
        if v.0 < -1e-9 || (v.0.abs() <= 1e-9 && v.1 < 0.0) {
            v = (-v.0, -v.1);
        }
        let along: Vec<f64> = ring.iter().map(|p| {
            let d = sub(*p, c);
            d.0 * u.0 + d.1 * u.1
        }).collect();
        let (t0, t1) = (along.iter().cloned().fold(f64::INFINITY, f64::min), along.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
        let plat_len = t1 - t0;
        let in_ring = |q: P| -> bool {
            let mut inside = false;
            for w in ring.windows(2) {
                let (a, b) = (w[0], w[1]);
                if (a.1 > q.1) != (b.1 > q.1) && q.0 < (b.0 - a.0) * (q.1 - a.1) / (b.1 - a.1) + a.0 {
                    inside = !inside;
                }
            }
            inside
        };
        // votes[side][edge]: side 0 = lateral negative (west), 1 = positive.
        let mut votes: [BTreeMap<usize, usize>; 2] = [BTreeMap::new(), BTreeMap::new()];
        for (i, ed) in yard.edges.iter().enumerate() {
            for q in samples(&ed.poly, PLATFORM_SAMPLE_M) {
                let d = sub(q, c);
                let t = d.0 * u.0 + d.1 * u.1;
                if t < t0 || t > t1 || in_ring(q) || poly_dist(q, &ring) > PLATFORM_REACH_M {
                    continue;
                }
                let side = if d.0 * v.0 + d.1 * v.1 >= 0.0 { 1 } else { 0 };
                *votes[side].entry(i).or_default() += 1;
            }
        }
        let best = |side: usize| -> Option<(usize, usize)> {
            votes[side].iter().max_by_key(|&(i, n)| (*n, std::cmp::Reverse(*i))).map(|(&i, &n)| (i, n))
        };
        let full = (plat_len / PLATFORM_SAMPLE_M).max(1.0);
        let chosen: Vec<Option<(usize, usize)>> = if refs.len() >= 2 {
            vec![best(0), best(1)]
        } else if best(0).map_or(0, |x| x.1) >= best(1).map_or(0, |x| x.1) {
            vec![best(0)]
        } else {
            vec![best(1)]
        };
        for (r, ch) in refs.iter().zip(chosen) {
            attaches.push(Attach {
                name: r.clone(),
                way: pw.id,
                edge: ch.map(|x| x.0),
                share: ch.map_or(0.0, |x| (x.1 as f64 / full).min(1.0)),
                len: plat_len,
            });
        }
    }
    for a in &attaches {
        if let Some(edge) = a.edge {
            yard.platforms.push(Platform { name: a.name.clone(), way: a.way, edge, len: a.len });
        }
    }

    // 6b. Promotion. OpenStreetMap tags most platform tracks, and the ladders
    // that lead to them, `service=siding` or `yard`. The game works running
    // lines and platforms, so any yard or siding way a train needs to get from
    // a portal to a platform track is read as running. Done by rule, not by
    // hand: every promoted way is listed in the report and written to the
    // `.yard` file, where the panel tags it assumed. A way no portal can reach
    // over any track is a hole in the data and is never promoted.
    let kinds_as_tagged = kinds_summary(&yard);
    let mut promoted: BTreeMap<u64, Kind> = BTreeMap::new();
    let mut holes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for _ in 0..8 {
        let mut changed = false;
        holes.clear();
        for a in &attaches {
            let Some(edge) = a.edge else { continue };
            for pi in 0..yard.portals.len() {
                let p = yard.portals[pi].clone();
                let start = (p.edge, yard.edges[p.edge].a == p.node);
                if !reach(&yard, &class, &inc, start, &|_| true).0.contains(&edge) {
                    holes.entry(a.name.clone()).or_default().push(p.name.clone());
                } else if !reach(&yard, &class, &inc, start, &|k| k == Kind::Running).0.contains(&edge) {
                    if let Some(ways) = cheapest_path(&yard, &class, &inc, start, edge) {
                        for w in ways {
                            for ed in yard.edges.iter_mut().filter(|e| e.way == w) {
                                promoted.entry(w).or_insert(ed.kind);
                                ed.kind = Kind::Running;
                            }
                            changed = true;
                        }
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    yard.promoted = promoted.iter().map(|(&w, &k)| (w, k)).collect();

    // 7. Reachability over running track.
    let running = |k: Kind| k == Kind::Running;
    let any_kind = |_: Kind| true;
    let portal_reach: Vec<BTreeSet<usize>> = yard
        .portals
        .iter()
        .map(|p| reach(&yard, &class, &inc, (p.edge, yard.edges[p.edge].a == p.node), &running).0)
        .collect();
    let portal_node_set: BTreeSet<usize> = yard.portals.iter().map(|p| p.node).collect();

    // ---- report --------------------------------------------------------------
    rep.say(format!("OpenStreetMap base: {}", osm.base));
    rep.say(format!(
        "ways: {} track in {} fragments inside the box ({} wholly outside), {} platform outlines ({} metro platforms skipped), {} unclassified (read as yard)",
        way_kind.len(), frags.len(), outside_ways, platform_ways, skipped_platforms, unclassified
    ));
    rep.say(format!("edges as tagged: {kinds_as_tagged}"));
    rep.say(format!(
        "edges after promotion: {}; {} ways promoted to running: {}",
        kinds_summary(&yard),
        promoted.len(),
        promoted.iter().map(|(w, k)| format!("{w} ({})", k.name())).collect::<Vec<_>>().join(", ")
    ));
    let degree: BTreeMap<usize, usize> = inc.iter().fold(BTreeMap::new(), |mut m, l| {
        *m.entry(l.len()).or_default() += 1;
        m
    });
    rep.say(format!("graph nodes: {} (by degree: {:?})", yard.nodes.len(), degree));

    // Connected components over all kinds, then over running only.
    let comps = |ok: &dyn Fn(Kind) -> bool| -> Vec<(usize, f64)> {
        let mut parent: Vec<usize> = (0..yard.nodes.len()).collect();
        fn find(p: &mut Vec<usize>, x: usize) -> usize {
            let mut r = x;
            while p[r] != r { r = p[r]; }
            let mut c = x;
            while p[c] != r { let nx = p[c]; p[c] = r; c = nx; }
            r
        }
        for ed in yard.edges.iter().filter(|e| ok(e.kind)) {
            let (a, b) = (find(&mut parent, ed.a), find(&mut parent, ed.b));
            parent[a] = b;
        }
        let mut m: BTreeMap<usize, (usize, f64)> = BTreeMap::new();
        for ed in yard.edges.iter().filter(|e| ok(e.kind)) {
            let r = find(&mut parent, ed.a);
            let v = m.entry(r).or_default();
            v.0 += 1;
            v.1 += ed.len;
        }
        let mut v: Vec<(usize, f64)> = m.into_values().collect();
        v.sort_by(|a, b| b.0.cmp(&a.0));
        v
    };
    let fmt = |v: &[(usize, f64)]| v.iter().take(8).map(|(c, l)| format!("{c} edges/{l:.0} m")).collect::<Vec<_>>().join(", ");
    let (ca, cr) = (comps(&any_kind), comps(&running));
    rep.say(format!("components, all track: {} ({})", ca.len(), fmt(&ca)));
    rep.say(format!("components, running only: {} ({})", cr.len(), fmt(&cr)));

    let switch_nodes: Vec<u64> = osm
        .nodes
        .iter()
        .filter(|(id, (_, _, t))| t.get("railway").map(|s| s.as_str()) == Some("switch") && inside(**id))
        .map(|(&id, _)| id)
        .collect();
    let at_point: BTreeSet<u64> = yard.points.iter().map(|p| yard.nodes[p.node].osm).collect();
    let switch_unmatched: Vec<u64> = {
        let mut v: Vec<u64> = switch_nodes.iter().copied().filter(|id| !at_point.contains(id)).collect();
        v.sort();
        v
    };
    let points_untagged = at_point.iter().filter(|id| !switch_nodes.contains(id)).count();
    rep.say(format!(
        "points: {} three-leg junctions; {} tagged railway=switch inside the box; {} switch tags sit where the graph has no three-leg junction (a missing or extra leg); {} junctions have no switch tag",
        yard.points.len(), switch_nodes.len(), switch_unmatched.len(), points_untagged
    ));
    if !switch_unmatched.is_empty() {
        let deg = |id: u64| index.get(&merged(id)).map_or("not a junction".to_string(), |&ni| format!("degree {}", inc[ni].len()));
        rep.say(format!(
            "  switch tags off a three-leg junction (first 12): {}",
            switch_unmatched.iter().take(12).map(|&id| format!("{id} ({})", deg(id))).collect::<Vec<_>>().join(", ")
        ));
    }
    let others: Vec<String> = class
        .iter()
        .enumerate()
        .filter_map(|(ni, c)| matches!(c, Class::Complex).then(|| format!("{} (degree {})", yard.nodes[ni].osm, inc[ni].len())))
        .collect();
    rep.say(format!(
        "points with other than 3 legs: {} junctions of degree 4 ({} tagged railway_crossing; {} tagged railway=switch, read as double slips (assumed): {}; {} untagged, read as diamonds), {} of degree 5 or more{}",
        yard.crossings.len(),
        yard.crossings.len() - untagged_cross.len() - slips.len(),
        slips.len(),
        slips.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(", "),
        untagged_cross.len(),
        complex.len(),
        if others.is_empty() { String::new() } else { format!(": {}", others.join(", ")) }
    ));
    rep.say(format!(
        "points with an unclear stem (best angle under {STEM_CONFIDENT_DEG} deg): {}{}",
        unclear_stem.len(),
        if unclear_stem.is_empty() { String::new() } else {
            format!(" ({})", unclear_stem.iter().take(10).map(|(id, a)| format!("{id} {a:.0} deg")).collect::<Vec<_>>().join(", "))
        }
    ));
    rep.say(format!(
        "points whose legs differ by under {LEG_AMBIGUOUS_DEG} deg from straight: {}{}",
        ambiguous_legs.len(),
        if ambiguous_legs.is_empty() { String::new() } else {
            format!(" ({})", ambiguous_legs.iter().take(10).map(|(id, a, b)| format!("{id} {a:.0}/{b:.0}")).collect::<Vec<_>>().join(", "))
        }
    ));
    let dead_untagged_in: Vec<String> = untagged_dead
        .iter()
        .filter(|ni| !portal_node_set.contains(ni))
        .map(|&ni| {
            let (e, _) = inc[ni].first().copied().unwrap_or((0, true));
            format!("{} ({})", yard.nodes[ni].osm, yard.edges.get(e).map_or("?", |ed| ed.kind.name()))
        })
        .collect();
    rep.say(format!(
        "dead ends: {} with a buffer stop; {} without one and not a portal{}",
        buffers,
        dead_untagged_in.len(),
        if dead_untagged_in.is_empty() { String::new() } else {
            format!(" (first 12: {})", dead_untagged_in.iter().take(12).cloned().collect::<Vec<_>>().join(", "))
        }
    ));
    rep.say(format!(
        "portals: {} running lines leave the box ({}); {} non-running tracks leave it and are not routed",
        yard.portals.len(),
        yard.portals.iter().map(|p| format!("{}={}", p.name, yard.nodes[p.node].osm)).collect::<Vec<_>>().join(", "),
        other_exits
    ));

    rep.say("platforms (name, way, attached track way/kind, votes share, platform length / track edge length):");
    for a in &attaches {
        match a.edge {
            Some(e) => {
                let ed = &yard.edges[e];
                let note = if a.share < PLATFORM_CONFIDENT_SHARE { "  <- weak: neighbouring tracks compete" } else { "" };
                let off = if ed.kind != Kind::Running { "  <- track is not Running" } else { "" };
                rep.say(format!(
                    "  {:<8} {:>11}  track {} {:<7} share {:.0}%  {:.0} m / {:.0} m{note}{off}",
                    a.name, a.way, ed.way, ed.kind.name(), a.share * 100.0, a.len, ed.len
                ));
            }
            None => rep.say(format!("  {:<8} {:>11}  NO TRACK within {PLATFORM_REACH_M} m", a.name, a.way)),
        }
    }

    // ---- the gate -----------------------------------------------------------
    // Every platform track is reached from at least one portal and can leave by
    // at least one, over running track, obeying points. Pairs that fail are
    // findings below, not failures: the plan asked for every portal to reach
    // every platform, which this graph cannot meet (see the holes line).
    let mut reach_rows = Vec::new();
    for a in &attaches {
        let Some(edge) = a.edge else {
            rep.failures.push(format!("platform {} (way {}) has no track within {PLATFORM_REACH_M} m", a.name, a.way));
            continue;
        };
        let arrive = portal_reach.iter().filter(|r| r.contains(&edge)).count();
        // Departure: can a train standing on this track, facing either way,
        // get out through a portal?
        let mut out: BTreeSet<usize> = BTreeSet::new();
        for fwd in [true, false] {
            out.extend(reach(&yard, &class, &inc, (edge, fwd), &running).1);
        }
        let leaves = yard.portals.iter().filter(|p| out.contains(&p.node)).count();
        reach_rows.push(format!(
            "  {:<8} reached from {arrive}/{} portals; leaves by {leaves}/{} portals",
            a.name,
            yard.portals.len(),
            yard.portals.len()
        ));
        if arrive == 0 {
            rep.failures.push(format!("platform {} (way {}) cannot be reached from any portal", a.name, a.way));
        }
        if leaves == 0 {
            rep.failures.push(format!("platform {} (way {}) cannot leave by any portal", a.name, a.way));
        }
    }
    rep.say("platform reachability over running track after promotion, obeying points:");
    rep.lines.extend(reach_rows);
    let hole_pairs: usize = holes.values().map(|v| v.len()).sum();
    rep.say(format!(
        "findings, not failures: {hole_pairs} portal-platform pairs have no path over any track, obeying points{}",
        if holes.is_empty() { String::new() } else {
            format!(
                ": {}",
                holes.iter().map(|(pl, ps)| format!("platform {pl} from {}", ps.join(","))).collect::<Vec<_>>().join("; ")
            )
        }
    ));
    if yard.portals.is_empty() {
        rep.failures.push("no portal found: no running line leaves the box".into());
    }
    if platform_ways == 0 {
        rep.failures.push("no platform found".into());
    }
    yard.signals = cabin_sim::signal::generate(&yard);
    apply_signal_overrides(&mut yard, &ov.signal)?;
    let yard = yard.quantised();
    report_signals(&yard, &mut rep);
    Ok((yard, rep))
}

/// Move and rename generated signals. Every line must name a signal that was
/// generated, and a move must stay on the signal's edge: crossing a junction
/// would change which routes it governs, which a move should not do quietly.
fn apply_signal_overrides(yard: &mut Yard, ov: &BTreeMap<String, SignalOverride>) -> Result<(), String> {
    for (name, so) in ov {
        let k = yard
            .signals
            .iter()
            .position(|s| &s.name == name)
            .ok_or(format!("overrides.toml: [signal] {name}: no generated signal has that name"))?;
        let len = yard.edges[yard.signals[k].edge].len;
        let s = &mut yard.signals[k];
        let to = s.offset + if s.forward { so.shift_m } else { -so.shift_m };
        if !(0.0..=len).contains(&to) {
            return Err(format!(
                "overrides.toml: [signal] {name}: shift_m {} leaves its {len:.0} m edge; move it less",
                so.shift_m
            ));
        }
        s.offset = to;
        if let Some(n) = &so.name {
            s.name = n.clone();
        }
        s.origin = Origin::Override;
    }
    let mut names: Vec<&str> = yard.signals.iter().map(|s| s.name.as_str()).collect();
    names.sort();
    if let Some(w) = names.windows(2).find(|w| w[0] == w[1]) {
        return Err(format!("overrides.toml: [signal]: two signals are named {}", w[0]));
    }
    Ok(())
}

/// B3's printed line: signals by kind and origin, the routes they give, the
/// longest, and every platform end or Home that has no route.
fn report_signals(yard: &Yard, rep: &mut Report) {
    use cabin_sim::route::{build_routes, Exit};
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    for s in &yard.signals {
        *by.entry(format!("{} {}", s.kind.name(), s.origin.name())).or_default() += 1;
    }
    rep.say(format!(
        "signals: {} ({}); every position is authored by rule, none comes from OpenStreetMap",
        yard.signals.len(),
        by.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", ")
    ));
    let routes = build_routes(yard);
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for r in &routes {
        let what = match r.exit {
            Exit::Berth { .. } => "to a berth",
            Exit::Portal(_) => "to a portal",
        };
        *kinds.entry(format!("{} {what}", yard.signals[r.entry].kind.name())).or_default() += 1;
    }
    rep.say(format!(
        "routes: {} ({})",
        routes.len(),
        kinds.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", ")
    ));
    let exit_name = |e: Exit| match e {
        Exit::Berth { starter, .. } => yard.signals[starter].name.clone(),
        Exit::Portal(p) => yard.portals[p].name.clone(),
    };
    if let Some(r) = routes.iter().max_by(|a, b| a.len_m.total_cmp(&b.len_m)) {
        rep.say(format!(
            "longest route: {} to {}, {:.0} m over {} edges, {} points, {} m of overlap edges",
            yard.signals[r.entry].name,
            exit_name(r.exit),
            r.len_m,
            r.path.len(),
            r.points.len(),
            r.overlap.iter().map(|&e| yard.edges[e].len).sum::<f64>().round()
        ));
    }
    let g = yard.graph();
    // A Starter whose line ahead is all yard or siding: its platform is
    // worked from the other end only, so this end has no departure and the
    // other end no arrival.
    let stub = |s: &Signal| {
        let ahead = g.next(yard, s.edge, s.forward);
        !ahead.is_empty() && ahead.iter().all(|&(e, _)| yard.edges[e].kind != Kind::Running)
    };
    let mut missing = Vec::new();
    for (k, s) in yard.signals.iter().enumerate() {
        let out = routes.iter().filter(|r| r.entry == k).count();
        let into = routes.iter().filter(|r| matches!(r.exit, Exit::Berth { starter, .. } if starter == k)).count();
        let why = if stub(s) { " (only yard or siding track beyond)" } else { "" };
        match s.kind {
            SignalKind::Home if out == 0 => missing.push(format!("{} sets no route", s.name)),
            SignalKind::Starter if into == 0 && out == 0 => missing.push(format!("{} no arrival, no departure{why}", s.name)),
            SignalKind::Starter if into == 0 => missing.push(format!("{} no arrival", s.name)),
            SignalKind::Starter if out == 0 => missing.push(format!("{} no departure{why}", s.name)),
            _ => {}
        }
    }
    for (k, pl) in yard.platforms.iter().enumerate() {
        if yard.signals.iter().filter(|s| s.platform == Some(k)).count() < 2 {
            missing.push(format!("platform {} has one Starter: its other end meets a buffer stop", pl.name));
        }
    }
    rep.say(format!(
        "platform ends and Homes with no route: {}{}",
        missing.len(),
        if missing.is_empty() { String::new() } else { format!(": {}", missing.join("; ")) }
    ));
}

// ---- Guard 2: never replace a good yard with a worse one ---------------------

/// (portal, platform) name pairs a train from the portal can reach, over
/// running track, obeying points and crossings. Read from the yard file's own
/// records, so an old yard on disk is measured the same way as a new one.
fn reached_pairs(y: &Yard) -> BTreeSet<(String, String)> {
    let g = y.graph();
    let mut out = BTreeSet::new();
    for p in &y.portals {
        let start = (p.edge, y.edges[p.edge].a == p.node);
        let mut seen = BTreeSet::from([start]);
        let mut todo = vec![start];
        while let Some((e, f)) = todo.pop() {
            for s in g.next(y, e, f) {
                if y.edges[s.0].kind == Kind::Running && seen.insert(s) {
                    todo.push(s);
                }
            }
        }
        for pl in &y.platforms {
            if seen.iter().any(|s| s.0 == pl.edge) {
                out.insert((p.name.clone(), pl.name.clone()));
            }
        }
    }
    out
}

/// What `new` lacks that `old` has, one line per kind of loss. Empty means
/// the new yard is no worse by these four measures.
fn losses(old: &Yard, new: &Yard) -> Vec<String> {
    fn lost<T: Ord + Clone + std::fmt::Display>(what: &str, a: BTreeSet<T>, b: BTreeSet<T>) -> Option<String> {
        let gone: Vec<String> = a.difference(&b).map(|x| x.to_string()).collect();
        (!gone.is_empty()).then(|| {
            let more = if gone.len() > 12 { format!(" and {} more", gone.len() - 12) } else { String::new() };
            format!("{} {what} lost: {}{more}", gone.len(), gone[..gone.len().min(12)].join(", "))
        })
    }
    let platforms = |y: &Yard| y.platforms.iter().map(|p| p.name.clone()).collect::<BTreeSet<_>>();
    let portals = |y: &Yard| y.portals.iter().map(|p| p.name.clone()).collect::<BTreeSet<_>>();
    let ways = |y: &Yard| y.edges.iter().map(|e| e.way).collect::<BTreeSet<_>>();
    let pairs = |y: &Yard| reached_pairs(y).into_iter().map(|(p, q)| format!("{p}->P{q}")).collect::<BTreeSet<_>>();
    [
        lost("platforms", platforms(old), platforms(new)),
        lost("portals", portals(old), portals(new)),
        lost("portal-to-platform paths", pairs(old), pairs(new)),
        lost("track ways", ways(old), ways(new)),
    ]
    .into_iter()
    .flatten()
    .collect()
}

// ---- run --------------------------------------------------------------------

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn run() -> Result<(Yard, Report, usize), String> {
    let dir = data_dir();
    let osm_text = std::fs::read_to_string(dir.join("ndls.osm.json"))
        .map_err(|e| format!("data/ndls.osm.json: {e}; run ./fetch-osm.sh first"))?;
    let ov_text = std::fs::read_to_string(dir.join("ndls.overrides.toml")).unwrap_or_default();
    let ov = parse_overrides(&ov_text)?;
    let (yard, rep) = extract(&parse_osm(&osm_text)?, &ov)?;
    Ok((yard, rep, ov.lines()))
}

fn main() {
    let (yard, rep, ov_lines) = match run() {
        Ok(x) => x,
        Err(e) => {
            eprintln!("cabin-osm: {e}; the old data/ndls.yard stays.");
            std::process::exit(1);
        }
    };
    for l in &rep.lines {
        println!("{l}");
    }
    println!("override lines in use: {ov_lines} (stop and ask above {MAX_OVERRIDE_LINES})");
    if !rep.failures.is_empty() {
        println!("\nGATE FAILED ({} platforms); the old data/ndls.yard stays:", rep.failures.len());
        for f in &rep.failures {
            println!("  {f}");
        }
        std::process::exit(1);
    }
    let dir = data_dir();
    let tmp = dir.join("ndls.yard.tmp");
    let text = yard.to_text();
    // The file the game will read must parse back to exactly this yard.
    if Yard::parse(&text).as_ref() != Ok(&yard) {
        eprintln!("cabin-osm: the yard did not survive a write and read back; nothing written.");
        std::process::exit(1);
    }
    // Guard 2. The gate above cannot see a loss; only a comparison can.
    let accept = std::env::args().any(|a| a == "--accept-worse");
    let refuse = |why: &str| {
        if accept {
            println!("\n--accept-worse: writing anyway. {why}");
        } else {
            println!("\nREFUSED: {why}\nThe old data/ndls.yard stays. If the change is meant, run: cargo run -p cabin-osm -- --accept-worse");
            std::process::exit(1);
        }
    };
    match std::fs::read_to_string(dir.join("ndls.yard")) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => refuse(&format!("data/ndls.yard could not be read ({e}), so nothing can be compared.")),
        Ok(old_text) => match Yard::parse(&old_text) {
            Err(e) => refuse(&format!("data/ndls.yard does not parse with this build ({}), so nothing can be compared.", e.0)),
            Ok(old) => {
                let l = losses(&old, &yard);
                if !l.is_empty() {
                    refuse(&format!("the new yard is worse than the one on disk:\n  {}", l.join("\n  ")));
                }
            }
        },
    }
    std::fs::write(&tmp, &text).expect("write ndls.yard.tmp");
    std::fs::rename(&tmp, dir.join("ndls.yard")).expect("rename ndls.yard");
    println!("\nGATE PASSED. wrote data/ndls.yard ({} bytes)", text.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Metres east and north of the box centre, as latitude and longitude.
    fn ll(x: f64, y: f64) -> (f64, f64) {
        let (lat0, lon0) = ((BBOX.0 + BBOX.2) / 2.0, (BBOX.1 + BBOX.3) / 2.0);
        (
            lat0 + (y / EARTH_R_M).to_degrees(),
            lon0 + (x / (EARTH_R_M * lat0.to_radians().cos())).to_degrees(),
        )
    }

    /// A point at the origin and two legs east: B straight on, C bending 7.6
    /// deg. `west_portal` lets the stem way leave the box to the west;
    /// `east_portal` lets leg B leave it to the east; `c_service` tags leg C
    /// (empty means a main line). A platform outline sits just south of C.
    fn synthetic(west_portal: bool, east_portal: bool, c_service: &str) -> String {
        let pts: [(u64, f64, f64); 12] = [
            (1, -1000.0, 0.0), // outside the box: cuts way A
            (2, -500.0, 0.0),
            (3, 0.0, 0.0), // the point
            (4, 300.0, 0.0),
            (5, 400.0, 0.0),
            (6, 300.0, -40.0),
            (7, 400.0, -40.0),
            (8, 1200.0, 0.0), // outside the box: cuts way B
            (9, 300.0, -50.0), // the platform outline, 4 m south of leg C
            (10, 400.0, -50.0),
            (11, 400.0, -44.0),
            (12, 300.0, -44.0),
        ];
        let mut els: Vec<String> = pts
            .iter()
            .map(|&(id, x, y)| {
                let (lat, lon) = ll(x, y);
                format!(r#"{{"type":"node","id":{id},"lat":{lat},"lon":{lon}}}"#)
            })
            .collect();
        let a = if west_portal { "[1,2,3]" } else { "[2,3]" };
        els.push(format!(r#"{{"type":"way","id":100,"nodes":{a},"tags":{{"railway":"rail","usage":"main"}}}}"#));
        let b = if east_portal { "[3,4,5,8]" } else { "[3,4,5]" };
        els.push(format!(r#"{{"type":"way","id":101,"nodes":{b},"tags":{{"railway":"rail","usage":"main"}}}}"#));
        let c_tags = if c_service.is_empty() {
            r#""railway":"rail","usage":"main""#.to_string()
        } else {
            format!(r#""railway":"rail","service":"{c_service}""#)
        };
        els.push(format!(r#"{{"type":"way","id":102,"nodes":[3,6,7],"tags":{{{c_tags}}}}}"#));
        els.push(r#"{"type":"way","id":200,"nodes":[9,10,11,12,9],"tags":{"railway":"platform","ref":"1"}}"#.into());
        format!(r#"{{"osm3s":{{"timestamp_osm_base":"test"}},"elements":[{}]}}"#, els.join(","))
    }

    fn run_synthetic(w: bool, e: bool, c: &str) -> (Yard, Report) {
        extract(&parse_osm(&synthetic(w, e, c)).unwrap(), &Overrides::default()).unwrap()
    }

    #[test]
    fn point_stem_and_legs_are_found_and_gate_passes() {
        let (yard, rep) = run_synthetic(true, false, "");
        assert_eq!(yard.points.len(), 1);
        let p = &yard.points[0];
        assert_eq!(yard.edges[p.stem].way, 100, "stem is the west way");
        assert_eq!(yard.edges[p.normal].way, 101, "the straight leg is normal");
        assert_eq!(yard.edges[p.reverse].way, 102, "the bending leg is reverse");
        assert_eq!(yard.portals.len(), 1);
        assert_eq!(yard.portals[0].name, "W1");
        assert_eq!(yard.platforms.len(), 1);
        assert_eq!(yard.edges[yard.platforms[0].edge].way, 102, "platform serves the nearer track");
        assert!(rep.failures.is_empty(), "{:?}", rep.failures);
        assert!(yard.promoted.is_empty());
    }

    #[test]
    fn a_platform_no_portal_can_reach_fails_the_gate() {
        // The only portal is on leg B. From there a train can only take the
        // stem, so the platform beside leg C is out of reach.
        let (_, rep) = run_synthetic(false, true, "");
        assert_eq!(rep.failures.len(), 2, "{:?}", rep.failures);
        assert!(rep.failures[0].contains("cannot be reached from any portal"), "{:?}", rep.failures);
        assert!(rep.failures[1].contains("cannot leave"), "{:?}", rep.failures);
    }

    #[test]
    fn one_portal_that_cannot_reach_a_platform_is_a_finding_not_a_failure() {
        // W1 reaches the platform, E1 cannot: the relaxed gate passes and the
        // pair is printed.
        let (_, rep) = run_synthetic(true, true, "");
        assert!(rep.failures.is_empty(), "{:?}", rep.failures);
        let line = rep.lines.iter().find(|l| l.starts_with("findings, not failures")).unwrap();
        assert!(line.contains("1 portal-platform pairs") && line.contains("platform 1 from E1"), "{line}");
    }

    #[test]
    fn a_siding_the_platform_needs_is_promoted_and_recorded() {
        let (yard, rep) = run_synthetic(true, false, "siding");
        assert!(rep.failures.is_empty(), "{:?}", rep.failures);
        assert_eq!(yard.promoted, vec![(102, Kind::Siding)]);
        assert!(yard.edges.iter().filter(|e| e.way == 102).all(|e| e.kind == Kind::Running));
        // The file must carry it, and read back identical.
        assert_eq!(Yard::parse(&yard.to_text()).as_ref(), Ok(&yard));
    }

    #[test]
    fn an_unneeded_siding_stays_a_siding() {
        // Leg C carries no platform here: drop the platform and C must not be promoted.
        let json = synthetic(true, false, "siding").replace(r#""railway":"platform""#, r#""railway":"disused""#);
        let (yard, _) = extract(&parse_osm(&json).unwrap(), &Overrides::default()).unwrap();
        assert!(yard.promoted.is_empty());
        assert!(yard.edges.iter().filter(|e| e.way == 102).all(|e| e.kind == Kind::Siding));
    }

    #[test]
    fn an_override_naming_a_missing_way_refuses_the_build() {
        let ov = parse_overrides("[kind]\n999 = \"yard\"\n").unwrap();
        let err = extract(&parse_osm(&synthetic(true, false, "")).unwrap(), &ov).err().unwrap();
        assert!(err.contains("way 999"), "{err}");
    }

    /// Two tracks crossing at 6 degrees at node 5, as every four-track node at
    /// NDLS does. Ways 100/101 run east; 102/103 run east-north-east.
    fn shallow_crossing(tag: &str) -> Yard {
        let pts = [(1, -300.0, 0.0), (2, 300.0, 0.0), (3, -300.0, -31.5), (4, 300.0, 31.5), (5, 0.0, 0.0)];
        let mut els: Vec<String> = pts
            .iter()
            .map(|&(id, x, y)| {
                let (lat, lon) = ll(x, y);
                let tags = if id == 5 { format!(r#","tags":{{"railway":"{tag}"}}"#) } else { String::new() };
                format!(r#"{{"type":"node","id":{id},"lat":{lat},"lon":{lon}{tags}}}"#)
            })
            .collect();
        for (id, a, b) in [(100, 1, 5), (101, 5, 2), (102, 3, 5), (103, 5, 4)] {
            els.push(format!(r#"{{"type":"way","id":{id},"nodes":[{a},{b}],"tags":{{"railway":"rail","usage":"main"}}}}"#));
        }
        let json = format!(r#"{{"osm3s":{{"timestamp_osm_base":"test"}},"elements":[{}]}}"#, els.join(","));
        extract(&parse_osm(&json).unwrap(), &Overrides::default()).unwrap().0
    }

    #[test]
    fn a_switch_tagged_crossing_is_a_slip_and_a_diamond_is_not() {
        let ways = |y: &Yard, n: usize| -> BTreeSet<(u64, u64)> {
            y.crossings[0].pairs[..n].iter().map(|&(a, b)| {
                let (p, q) = (y.edges[a].way, y.edges[b].way);
                (p.min(q), p.max(q))
            }).collect()
        };
        let straight = BTreeSet::from([(100, 101), (102, 103)]);
        let d = shallow_crossing("railway_crossing");
        assert_eq!(ways(&d, d.crossings[0].pairs.len()), straight, "a diamond goes straight over only");
        let s = shallow_crossing("switch");
        let mut slip = straight.clone();
        slip.extend([(100, 103), (101, 102)]);
        assert_eq!(ways(&s, s.crossings[0].pairs.len()), slip, "a slip adds the two diverging moves, never the 6-degree acute ones");
        assert_eq!(ways(&s, 2), straight, "straight pairs come first, so an overlap runs straight");
        assert_eq!(Yard::parse(&s.to_text()).as_ref(), Ok(&s), "a slip survives the file");
    }

    #[test]
    fn a_worse_yard_is_caught_and_an_equal_one_is_not() {
        let (old, _) = run_synthetic(true, true, "");
        assert!(losses(&old, &old).is_empty());
        // A portal gone, as when a line is cut from the answer.
        let (no_east, _) = run_synthetic(true, false, "");
        let l = losses(&old, &no_east);
        assert!(l.iter().any(|x| x.starts_with("1 portals lost: E1")), "{l:?}");
        // A platform outline gone, as in a truncated Overpass answer.
        let json = synthetic(true, true, "").replace(r#""railway":"platform""#, r#""railway":"disused""#);
        let (no_platform, _) = extract(&parse_osm(&json).unwrap(), &Overrides::default()).unwrap();
        let l = losses(&old, &no_platform);
        assert!(l.iter().any(|x| x.starts_with("1 platforms lost: 1")), "{l:?}");
        assert!(l.iter().any(|x| x.starts_with("1 portal-to-platform paths lost: W1->P1")), "{l:?}");
        // A track way gone: leg C, which the platform stands beside.
        let json = synthetic(true, true, "").replace(r#""id":102,"nodes":[3,6,7]"#, r#""id":102,"nodes":[]"#);
        let (no_leg, _) = extract(&parse_osm(&json).unwrap(), &Overrides::default()).unwrap();
        assert!(losses(&old, &no_leg).iter().any(|x| x.starts_with("1 track ways lost: 102")));
    }

    #[test]
    fn drop_false_is_refused_not_read_as_drop() {
        assert!(parse_overrides("[drop]\n100 = false\n").unwrap_err().contains("must be true"));
        assert!(parse_overrides("[drop]\n100 = true\n").unwrap().drop.contains(&100));
    }

    #[test]
    fn signals_are_generated_and_an_override_moves_and_renames_one() {
        let (yard, _) = run_synthetic(true, false, "");
        let names: Vec<&str> = yard.signals.iter().map(|s| s.name.as_str()).collect();
        // The platform track ends in a buffer stop to the east: one Starter.
        assert_eq!(names, vec!["HW1", "ST1W"], "{names:?}");
        assert!(yard.signals.iter().all(|s| s.origin == Origin::Generated));
        let before = yard.signals[0].offset;
        let ov = parse_overrides("[signal]\nHW1 = { shift_m = -20.0, name = \"W1H\" }\n").unwrap();
        let (moved, _) = extract(&parse_osm(&synthetic(true, false, "")).unwrap(), &ov).unwrap();
        let s = &moved.signals[0];
        assert_eq!((s.name.as_str(), s.origin), ("W1H", Origin::Override));
        // HW1 faces east (forward) on the west way, so -20 m moves it west.
        assert!(s.forward && (s.offset - (before - 20.0)).abs() < 0.05, "{} -> {}", before, s.offset);
    }

    #[test]
    fn a_signal_override_naming_no_signal_or_leaving_its_edge_refuses_the_build() {
        let osm = parse_osm(&synthetic(true, false, "")).unwrap();
        let ov = parse_overrides("[signal]\nHX9 = { shift_m = 5 }\n").unwrap();
        assert!(extract(&osm, &ov).err().unwrap().contains("no generated signal"));
        let ov = parse_overrides("[signal]\nHW1 = { shift_m = 5000 }\n").unwrap();
        assert!(extract(&osm, &ov).err().unwrap().contains("leaves its"));
    }

    /// B1 done-when: every value in the traffic profile says where it came
    /// from or that it is assumed, and every row id it cites exists.
    #[test]
    fn traffic_values_are_sourced_and_their_rows_exist() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ids: BTreeSet<String> = BTreeSet::new();
        for f in ["traffic-volume.md", "traffic-pool.md", "signalling-search.md"] {
            let text = std::fs::read_to_string(root.join("research").join(f)).unwrap();
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("| ") {
                    let id = rest.split(' ').next().unwrap_or("");
                    if id.len() > 2 && id[1..2] == *"-" && id[2..].chars().all(|c| c.is_ascii_digit()) {
                        ids.insert(id.to_string());
                    }
                }
            }
        }
        assert!(ids.len() > 150, "ledgers not found or empty: {} rows", ids.len());
        let text = std::fs::read_to_string(root.join("data/ndls.traffic.toml")).unwrap();
        let doc: toml::Table = text.parse().unwrap();
        let tags = ["primary", "secondary", "derived", "assumed", "unconfirmed"];
        let mut checked = 0;
        let mut check = |what: &str, t: &toml::Table| {
            let tag = t.get("tag").and_then(|x| x.as_str()).unwrap_or_else(|| panic!("{what}: no tag"));
            assert!(tags.contains(&tag), "{what}: tag '{tag}'");
            let src = t.get("src").and_then(|x| x.as_str()).unwrap_or_else(|| panic!("{what}: no src"));
            if src.is_empty() {
                assert!(tag == "assumed" || tag == "derived", "{what}: no source and not assumed");
            }
            for id in src.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()) {
                assert!(ids.contains(id), "{what}: cites {id}, which no ledger has");
            }
            checked += 1;
        };
        for (section, body) in &doc {
            if section == "train" {
                for (i, t) in body.as_array().unwrap().iter().enumerate() {
                    check(&format!("train #{i}"), t.as_table().unwrap());
                }
            } else {
                for (k, v) in body.as_table().unwrap() {
                    check(&format!("{section}.{k}"), v.as_table().unwrap_or_else(|| panic!("{section}.{k} is not a {{v, src, tag}} table")));
                }
            }
        }
        assert!(checked > 150, "only {checked} values checked");
        // The mix is shares of one whole.
        let m = |k: &str| doc["mix"][k]["v"].as_float().unwrap();
        assert!((m("emu_local") + m("express") + m("premium") - 1.0).abs() < 1e-9);
        assert!((m("rajdhani") + m("shatabdi") + m("duronto") - m("premium")).abs() < 1e-9);
    }

    /// The committed `.yard` must be what the committed OSM answer produces.
    #[test]
    fn committed_yard_matches_its_sources() {
        let dir = data_dir();
        let Ok(committed) = std::fs::read_to_string(dir.join("ndls.yard")) else { return };
        let (yard, rep, _) = run().unwrap();
        assert!(rep.failures.is_empty(), "{:?}", rep.failures);
        assert_eq!(yard.to_text(), committed, "data/ndls.yard is stale; run cargo run -p cabin-osm");
    }
}
