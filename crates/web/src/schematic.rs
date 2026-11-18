//! CABIN-WEB · SCHEMATIC LAYOUT
//!   · turns running and siding topology into horizontal rows and 45° jogs
//!   · leaves yard-only edges empty and gives them short approach stubs
//!   · geometry is for the panel; the yard remains the source of connections

use cabin_sim::yard::{Kind, Yard};
use std::collections::{BTreeMap, BTreeSet};

/// Tracks are separated enough to read as distinct lines at normal zoom.
const ROW_M: f64 = 30.0;
/// Drawn metres per surveyed metre along the axis: true length between the
/// first and last platform end, a third of it in the throats. A panel is read
/// at the platforms; at 0.35 throughout, P6 (435 m) drew 222 m wide while
/// one 9-row jog in a throat drew 270 m.
const ALONG_STATION: f64 = 1.0;
const ALONG: f64 = 0.35;
/// Nodes this close along the axis share one x, unless an edge joins them:
/// one jog's width needs to push the next node right, and every later node
/// moved with it, so platform ends 2 m apart in the yard drew 300 m apart.
const COLUMN_M: f64 = 6.0;
/// Leave visible flat before/after a 45° change of row.
const JOG_PAD_M: f64 = 8.0;
/// Geography gives ordering only; these reproduce Panel's stable long axis.
const SAMPLE_M: f64 = 10.0;
const BIN_M: f64 = 50.0;
const SMOOTH_BINS: usize = 2;
const ORDER_OVERLAP_M: f64 = 20.0;
const ROW_CLEAR_M: f64 = 40.0;
const EPS: f64 = 1e-6;

pub(crate) struct Schematic {
    /// Panel position of every node the layout draws; None for yard-only nodes.
    pub node: Vec<Option<(f64, f64)>>,
    /// One polyline per edge in panel metres; empty for a yard edge.
    pub shape: Vec<Vec<(f64, f64)>>,
    /// Short stubs where a yard edge leaves a drawn node.
    pub stubs: Vec<[(f64, f64); 2]>,
}

impl Schematic {
    pub(crate) fn new(yard: &Yard) -> Schematic {
        let axis = Axis::new(yard);
        let drawn: Vec<usize> = yard.edges.iter().enumerate()
            .filter_map(|(i, e)| matches!(e.kind, Kind::Running | Kind::Siding).then_some(i))
            .collect();
        let mut uf = UnionFind::new(yard.edges.len());
        let mut inc = vec![Vec::new(); yard.nodes.len()];
        for &e in &drawn {
            inc[yard.edges[e].a].push(e);
            inc[yard.edges[e].b].push(e);
        }
        let mut reverse_ends = BTreeSet::new();
        let point_nodes: BTreeSet<usize> = yard.points.iter().map(|p| p.node).collect();
        let crossing_nodes: BTreeSet<usize> = yard.crossings.iter().map(|c| c.node).collect();
        for point in &yard.points {
            uf.join(point.stem, point.normal);
            reverse_ends.insert((point.reverse, point.node));
        }
        // Lines are not joined through a diamond: two tracks that cross there
        // swap sides, and a line on one row can never cross another. Joined,
        // they left 22 false crossings on NDLS; split, each half takes its own
        // row and the diamond draws as an X of jogs.
        for n in 0..inc.len() {
            if !point_nodes.contains(&n) && !crossing_nodes.contains(&n) && inc[n].len() == 2 {
                uf.join(inc[n][0], inc[n][1]);
            }
        }

        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for &e in &drawn { groups.entry(uf.find(e)).or_default().push(e); }
        let platforms: BTreeSet<usize> = yard.platforms.iter().map(|p| p.edge).collect();
        let crossover_roots: BTreeSet<usize> = groups.iter().filter_map(|(&root, edges)| {
            (edges.len() == 1 && !platforms.contains(&edges[0])
                && reverse_ends.contains(&(edges[0], yard.edges[edges[0]].a))
                && reverse_ends.contains(&(edges[0], yard.edges[edges[0]].b))).then_some(root)
        }).collect();
        // The prototype walks edges in yard order, so preserve each line's first-edge order.
        // First-fit row placement changes when tied lines are ordered by union-find root.
        let mut roots: Vec<usize> = groups.keys().copied().filter(|r| !crossover_roots.contains(r)).collect();
        roots.sort_by_key(|r| groups[r][0]);
        let mut samples = BTreeMap::new();
        let mut spans = BTreeMap::new();
        for &root in &roots {
            let mut points: Vec<(f64, f64)> = groups[&root].iter()
                .flat_map(|&e| densify(&yard.edges[e].poly, SAMPLE_M))
                .map(|p| axis.order(p.0, p.1)).collect();
            points.sort_by(|a, b| a.0.total_cmp(&b.0));
            let span = (points.first().map_or(0.0, |p| p.0), points.last().map_or(0.0, |p| p.0));
            samples.insert(root, points);
            spans.insert(root, span);
        }
        let mut before: BTreeMap<usize, BTreeSet<usize>> = roots.iter().map(|&r| (r, BTreeSet::new())).collect();
        for i in 0..roots.len() {
            for j in i + 1..roots.len() {
                let (a, b) = (roots[i], roots[j]);
                let lo = spans[&a].0.max(spans[&b].0);
                let hi = spans[&a].1.min(spans[&b].1);
                if hi - lo < ORDER_OVERLAP_M { continue; }
                let diffs: Vec<f64> = (0..9).map(|k| {
                    let u = lo + (hi - lo) * k as f64 / 8.0;
                    nearest_v(&samples[&a], u) - nearest_v(&samples[&b], u)
                }).collect();
                let positive = diffs.iter().filter(|&&d| d > 0.0).count();
                let negative = diffs.iter().filter(|&&d| d < 0.0).count();
                let a_above = positive >= negative;
                if a_above { before.get_mut(&a).unwrap().insert(b); }
                else { before.get_mut(&b).unwrap().insert(a); }
            }
        }
        // A crossover says which side its far line is on where it joins it.
        // That beats the majority vote, which samples the whole shared span:
        // a line can run below another for 700 m and still be joined from
        // above by a crossover beyond the other's end (edge 107, way 77601829).
        // Each override is kept only if the order stays free of cycles.
        let line_at = |n: usize| inc[n].iter().copied()
            .filter(|&e| !reverse_ends.contains(&(e, n)))
            .find_map(|e| roots.iter().copied().find(|r| groups[r].contains(&e)));
        for c in &crossover_roots {
            let edge = &yard.edges[groups[c][0]];
            let (Some(la), Some(lb)) = (line_at(edge.a), line_at(edge.b)) else { continue };
            if la == lb { continue; }
            let v = |n: usize| axis.order(yard.nodes[n].x, yard.nodes[n].y).1;
            let (hi, lo) = if v(edge.b) > v(edge.a) { (lb, la) } else { (la, lb) };
            let mut sided = before.clone();
            sided.get_mut(&lo).unwrap().remove(&hi);
            sided.get_mut(&hi).unwrap().insert(lo);
            if topo_order(&roots, &sided).is_ok() { before = sided; }
        }
        let order = topo_order(&roots, &before).unwrap_or_else(|cycle| {
            panic!("schematic row ordering cycle among through-lines {cycle:?}")
        });
        let row = allocate_rows(&order, &before, &spans);
        let node_row = node_rows(yard, &inc, &groups, &crossover_roots, &reverse_ends, &row, &spans);
        let geom = geometry(yard, &axis, &drawn, &groups, &crossover_roots, &reverse_ends, &row, &node_row);

        let mut stubs = Vec::new();
        for edge in yard.edges.iter().filter(|e| e.kind == Kind::Yard) {
            for n in [edge.a, edge.b] {
                if let Some(at) = geom.0[n] {
                    let other = if n == edge.a { edge.b } else { edge.a };
                    let away = axis.rotate(yard.nodes[other].x, yard.nodes[other].y).1
                        - axis.rotate(yard.nodes[n].x, yard.nodes[n].y).1;
                    let dy = if away < 0.0 { -1.0 } else { 1.0 } * ROW_M * 0.6 / 2.0_f64.sqrt();
                    let dx = if axis.u[other] < axis.u[n] { -1.0 } else { 1.0 } * dy.abs();
                    let signed_dy = dy.signum() * dy.abs();
                    stubs.push([(at.0, at.1), (at.0 + dx, at.1 + signed_dy)]);
                }
            }
        }
        Schematic { node: geom.0, shape: geom.1, stubs }
    }
}

struct Axis {
    ox: f64,
    oy: f64,
    cos: f64,
    sin: f64,
    u0: f64,
    centre: Vec<f64>,
    u: Vec<f64>,
}

impl Axis {
    fn new(yard: &Yard) -> Axis {
        let platforms: Vec<&[(f64, f64)]> = yard.platforms.iter().map(|p| yard.edges[p.edge].poly.as_slice()).collect();
        let count = platforms.iter().map(|p| p.len()).sum::<usize>().max(1) as f64;
        let ox = platforms.iter().flat_map(|p| p.iter()).map(|p| p.0).sum::<f64>() / count;
        let oy = platforms.iter().flat_map(|p| p.iter()).map(|p| p.1).sum::<f64>() / count;
        let (mut c2, mut s2) = (0.0, 0.0);
        for w in platforms.iter().flat_map(|p| p.windows(2)) {
            let (dx, dy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
            let phi = dy.atan2(dx);
            let len = dx.hypot(dy);
            c2 += len * (2.0 * phi).cos();
            s2 += len * (2.0 * phi).sin();
        }
        let theta = 0.5 * s2.atan2(c2);
        let (cos, sin) = (theta.cos(), theta.sin());
        let rotate = |x: f64, y: f64| {
            let (dx, dy) = (x - ox, y - oy);
            (dx * cos + dy * sin, -dx * sin + dy * cos)
        };
        let mut samples: Vec<(f64, f64)> = yard.edges.iter().filter(|e| e.kind == Kind::Running)
            .flat_map(|e| densify(&e.poly, SAMPLE_M)).map(|p| rotate(p.0, p.1)).collect();
        let (umin, umax) = if samples.is_empty() { (0.0, 0.0) } else {
            (samples.iter().map(|p| p.0).fold(f64::INFINITY, f64::min), samples.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max))
        };
        let bins = ((umax - umin) / BIN_M).floor() as usize + 1;
        let mut by_bin = vec![Vec::new(); bins];
        for (u, v) in samples.drain(..) { by_bin[(((u - umin) / BIN_M) as usize).min(bins - 1)].push(v); }
        let mut med: Vec<Option<f64>> = by_bin.into_iter().map(|mut values| {
            values.sort_by(f64::total_cmp);
            (!values.is_empty()).then(|| values[values.len() / 2])
        }).collect();
        for i in 0..bins {
            if med[i].is_none() {
                med[i] = (1..bins).find_map(|d| i.checked_sub(d).and_then(|j| med[j]).or_else(|| med.get(i + d).copied().flatten()));
            }
        }
        let med: Vec<f64> = med.into_iter().map(|v| v.unwrap_or(0.0)).collect();
        let centre = (0..bins).map(|i| {
            let (a, b) = (i.saturating_sub(SMOOTH_BINS), (i + SMOOTH_BINS).min(bins - 1));
            med[a..=b].iter().sum::<f64>() / (b - a + 1) as f64
        }).collect();
        let mut axis = Axis { ox, oy, cos, sin, u0: umin + BIN_M / 2.0, centre, u: vec![0.0; yard.nodes.len()] };
        for (i, n) in yard.nodes.iter().enumerate() { axis.u[i] = axis.rotate(n.x, n.y).0; }
        axis
    }

    fn rotate(&self, x: f64, y: f64) -> (f64, f64) {
        let (dx, dy) = (x - self.ox, y - self.oy);
        (dx * self.cos + dy * self.sin, -dx * self.sin + dy * self.cos)
    }

    fn order(&self, x: f64, y: f64) -> (f64, f64) {
        let (u, v) = self.rotate(x, y);
        let t = ((u - self.u0) / BIN_M).clamp(0.0, (self.centre.len() - 1) as f64);
        let i = t.floor() as usize;
        let j = (i + 1).min(self.centre.len() - 1);
        (u, v - (self.centre[i] + (self.centre[j] - self.centre[i]) * (t - i as f64)))
    }
}

fn densify(poly: &[(f64, f64)], step: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    if let Some(&first) = poly.first() { out.push(first); }
    for w in poly.windows(2) {
        let pieces = ((w[1].0 - w[0].0).hypot(w[1].1 - w[0].1) / step).ceil().max(1.0) as usize;
        for i in 1..=pieces {
            let t = i as f64 / pieces as f64;
            out.push((w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t));
        }
    }
    out
}

fn nearest_v(points: &[(f64, f64)], u: f64) -> f64 {
    points.iter().min_by(|a, b| (a.0 - u).abs().total_cmp(&(b.0 - u).abs())).map_or(0.0, |p| p.1)
}

struct UnionFind(Vec<usize>);
impl UnionFind {
    fn new(n: usize) -> UnionFind { UnionFind((0..n).collect()) }
    fn find(&mut self, x: usize) -> usize {
        if self.0[x] != x { self.0[x] = self.find(self.0[x]); }
        self.0[x]
    }
    fn join(&mut self, a: usize, b: usize) { let (a, b) = (self.find(a), self.find(b)); self.0[a] = b; }
}

fn topo_order(roots: &[usize], before: &BTreeMap<usize, BTreeSet<usize>>) -> Result<Vec<usize>, Vec<usize>> {
    use std::collections::VecDeque;
    let mut indegree: BTreeMap<usize, usize> = roots.iter().map(|&r| (r, 0)).collect();
    let mut after: BTreeMap<usize, Vec<usize>> = roots.iter().map(|&r| (r, Vec::new())).collect();
    for (&a, bs) in before {
        for &b in bs { *indegree.get_mut(&a).unwrap() += 1; after.get_mut(&b).unwrap().push(a); }
    }
    let mut ready: VecDeque<usize> = roots.iter().filter(|&&r| indegree[&r] == 0).copied().collect();
    let mut order = Vec::new();
    while let Some(r) = ready.pop_front() {
        order.push(r);
        for &n in &after[&r] {
            let d = indegree.get_mut(&n).unwrap(); *d -= 1;
            if *d == 0 { ready.push_back(n); }
        }
    }
    if order.len() == roots.len() { Ok(order) } else { Err(indegree.into_iter().filter_map(|(r, d)| (d > 0).then_some(r)).collect()) }
}

fn allocate_rows(order: &[usize], before: &BTreeMap<usize, BTreeSet<usize>>, spans: &BTreeMap<usize, (f64, f64)>) -> BTreeMap<usize, usize> {
    let mut rows = BTreeMap::new();
    for &r in order {
        let mut row = before[&r].iter().filter_map(|b| rows.get(b).map(|v| v + 1)).max().unwrap_or(0);
        while rows.iter().any(|(other, &at): (&usize, &usize)| at == row && close_spans(spans[&r], spans[other])) { row += 1; }
        rows.insert(r, row);
    }
    rows
}

fn close_spans(a: (f64, f64), b: (f64, f64)) -> bool { !(a.1 + ROW_CLEAR_M < b.0 || b.1 + ROW_CLEAR_M < a.0) }

fn node_rows(yard: &Yard, inc: &[Vec<usize>], groups: &BTreeMap<usize, Vec<usize>>, crossovers: &BTreeSet<usize>, reverse: &BTreeSet<(usize, usize)>, rows: &BTreeMap<usize, usize>, spans: &BTreeMap<usize, (f64, f64)>) -> Vec<Option<usize>> {
    let mut root_of = vec![None; yard.edges.len()];
    for (&root, es) in groups { for &e in es { root_of[e] = Some(root); } }
    (0..yard.nodes.len()).map(|n| {
        let platform_edges: BTreeSet<usize> = yard.platforms.iter().map(|p| p.edge).collect();
        inc[n].iter().find(|&&e| platform_edges.contains(&e) && !reverse.contains(&(e, n)) && root_of[e].is_some_and(|r| !crossovers.contains(&r)))
            // At a diamond or a line's end, the longest line through the node
            // owns it; a 20 m line between two diamonds never does.
            .or_else(|| inc[n].iter().filter(|&&e| !reverse.contains(&(e, n)) && root_of[e].is_some_and(|r| !crossovers.contains(&r)))
                .max_by(|&&p, &&q| {
                    let len = |e: usize| spans.get(&root_of[e].unwrap()).map_or(0.0, |s| s.1 - s.0);
                    len(p).total_cmp(&len(q)).then(q.cmp(&p))
                }))
            .and_then(|&e| root_of[e]).and_then(|r| rows.get(&r).copied())
            .or_else(|| inc[n].iter().filter_map(|&e| root_of[e].and_then(|r| rows.get(&r).copied())).next())
    }).collect()
}

/// Node positions, edge shapes, and each drawn node's x in axis order.
type Geom = (Vec<Option<(f64, f64)>>, Vec<Vec<(f64, f64)>>, Vec<(usize, f64)>);

fn geometry(yard: &Yard, axis: &Axis, drawn: &[usize], groups: &BTreeMap<usize, Vec<usize>>, _crossovers: &BTreeSet<usize>, reverse: &BTreeSet<(usize, usize)>, rows: &BTreeMap<usize, usize>, node_rows: &[Option<usize>]) -> Geom {
    let mut edge_row = vec![None; yard.edges.len()];
    for (root, es) in groups { for &e in es { edge_row[e] = rows.get(root).copied(); } }
    // A node takes its row from one line. An edge that is not that line (a
    // reverse leg, the far side of a diamond) is foreign at that node, and
    // leaves it at 45° at once, as a turnout does.
    let foreign = |e: usize, n: usize| reverse.contains(&(e, n)) || edge_row[e].is_none() || node_rows[n] != edge_row[e];
    let mut drawn_nodes: BTreeSet<usize> = BTreeSet::new();
    for &e in drawn { drawn_nodes.insert(yard.edges[e].a); drawn_nodes.insert(yard.edges[e].b); }
    let mut ordered: Vec<usize> = drawn_nodes.iter().copied().collect();
    ordered.sort_by(|&a, &b| axis.u[a].total_cmp(&axis.u[b]).then(a.cmp(&b)));
    let mut x = vec![0.0; yard.nodes.len()];
    let mut position = BTreeMap::new();
    for (i, &n) in ordered.iter().enumerate() { position.insert(n, i); }
    let mut need: Vec<Vec<(usize, f64)>> = vec![Vec::new(); yard.nodes.len()];
    let platform_edges: BTreeSet<usize> = yard.platforms.iter().map(|p| p.edge).collect();
    for &e in drawn {
        let edge = &yard.edges[e];
        let (a, b) = (edge.a, edge.b);
        let (Some(ra), Some(rb)) = (node_rows[a], node_rows[b]) else { continue };
        if ra == rb { continue; }
        let (lo, hi) = if position[&a] < position[&b] { (a, b) } else { (b, a) };
        // Foreign at both ends: the edge is one diagonal with no flat, else
        // its flat would lie over the track that owns one of those nodes.
        let pad = if foreign(e, a) && foreign(e, b) { 0.0 } else { JOG_PAD_M };
        let jog = (ra.abs_diff(rb) as f64) * ROW_M;
        // A platform reads as a platform only if its flat run is at least as
        // long as its jog (P6: 435 m, compressed to 222 m, jogs 4 rows).
        let width = if platform_edges.contains(&e) { 2.0 * jog + pad } else { jog + pad };
        need[hi].push((lo, width));
    }
    let (z0, z1) = yard.platforms.iter().flat_map(|p| [axis.u[yard.edges[p.edge].a], axis.u[yard.edges[p.edge].b]])
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), u| (lo.min(u), hi.max(u)));
    // Drawn length of the axis between u0 and u1: ALONG_STATION inside [z0, z1].
    let along = |u0: f64, u1: f64| {
        let inside = (u1.min(z1) - u0.max(z0)).max(0.0);
        ALONG_STATION * inside + ALONG * ((u1 - u0).max(0.0) - inside)
    };
    let joined = |a: usize, b: usize| drawn.iter().any(|&e| {
        let edge = &yard.edges[e];
        (edge.a == a && edge.b == b) || (edge.a == b && edge.b == a)
    });
    let mut column: Vec<usize> = Vec::new();
    for k in 0..ordered.len() {
        let n = ordered[k];
        let fits = column.first().is_some_and(|&c| axis.u[n] - axis.u[c] <= COLUMN_M)
            && column.iter().all(|&c| !joined(c, n) && !need[n].iter().any(|&(a, _)| a == c));
        if fits {
            column.push(n);
        } else {
            column = vec![n];
            if k > 0 {
                let prev = ordered[k - 1];
                x[n] = x[prev] + along(axis.u[prev], axis.u[n]);
            }
        }
        // A column's x is the largest any member needs; members move together.
        let at = need[n].iter().map(|&(a, width)| x[a] + width).fold(x[column[0]], f64::max);
        for &c in &column { x[c] = at; }
    }
    let mut node = vec![None; yard.nodes.len()];
    for &n in &ordered { node[n] = node_rows[n].map(|r| (x[n], r as f64 * ROW_M)); }
    let mut shapes = vec![Vec::new(); yard.edges.len()];
    let mut jogs = Vec::new();
    for &e in drawn {
        let edge = &yard.edges[e];
        let (Some((xa, ya)), Some((xb, yb))) = (node[edge.a], node[edge.b]) else { continue };
        if (ya - yb).abs() < EPS { shapes[e] = vec![(xa, ya), (xb, yb)]; } else { jogs.push(e); }
    }
    // Flat edges are fixed by their nodes; each jog then picks where along its
    // edge to change rows. Short jogs first: they have the least freedom.
    jogs.sort_by(|&p, &q| {
        let d = |e: usize| (node[yard.edges[e].a].unwrap().1 - node[yard.edges[e].b].unwrap().1).abs();
        d(p).total_cmp(&d(q)).then(p.cmp(&q))
    });
    // The second pass re-places each jog with every other shape drawn, so a
    // short jog placed early can step aside for a long one placed after it.
    for e in jogs.iter().chain(jogs.iter()).copied() {
        let edge = &yard.edges[e];
        let (pa, pb) = (node[edge.a].unwrap(), node[edge.b].unwrap());
        // The jog goes at the foreign end; elsewhere only if that end would
        // cross or overlie a track.
        let foreign = |n: usize| foreign(e, n);
        let (left, right, fl, fr) = if pa.0 <= pb.0 { (pa, pb, foreign(edge.a), foreign(edge.b)) } else { (pb, pa, foreign(edge.b), foreign(edge.a)) };
        let d = (right.1 - left.1).abs();
        let (lo, hi) = (left.0, (right.0 - d).max(left.0));
        let preferred = if fl && !fr { lo } else if fr && !fl { hi } else { (lo + hi) / 2.0 };
        let mut candidates = vec![preferred, lo, hi, (lo + hi) / 2.0];
        for (o, sh) in shapes.iter().enumerate() {
            if sh.is_empty() || shares_node(yard, e, o) { continue; }
            for p in sh {
                for k in 0..=((d / ROW_M).round() as usize) {
                    let at = p.0 - k as f64 * ROW_M;
                    candidates.extend([at - 1.0, at + 1.0]);
                }
            }
        }
        let build = |s: f64| -> Vec<(f64, f64)> {
            let s = s.clamp(lo, hi);
            let mut v = vec![left];
            if s > lo + EPS { v.push((s, left.1)); }
            if s + d < right.0 - EPS { v.push((s + d, right.1)); }
            v.push(right);
            if pa.0 > pb.0 { v.reverse(); }
            v
        };
        let cost = |sh: &Vec<(f64, f64)>| -> usize {
            shapes.iter().enumerate().filter(|(o, other)| !other.is_empty() && !shares_node(yard, e, *o)
                && (polylines_cross(sh, other) || runs_overlap(sh, other))).count()
                + shapes.iter().enumerate().filter(|(o, other)| !other.is_empty() && *o != e && shares_node(yard, e, *o) && runs_overlap(sh, other)).count()
        };
        let mut best = (usize::MAX, f64::INFINITY, Vec::new());
        for s in candidates {
            if s < lo - EPS || s > hi + EPS { continue; }
            let sh = build(s);
            let c = cost(&sh);
            let away = (s - preferred).abs();
            if (c, away) < (best.0, best.1) { best = (c, away, sh); }
            if best.0 == 0 && best.1 == 0.0 { break; }
        }
        shapes[e] = best.2;
    }
    (node, shapes, ordered.into_iter().map(|n| (n, x[n])).collect())
}

fn shares_node(yard: &Yard, a: usize, b: usize) -> bool {
    let (p, q) = (&yard.edges[a], &yard.edges[b]);
    [p.a, p.b].iter().any(|n| *n == q.a || *n == q.b)
}

/// Two shapes run along the same row for more than 1 m.
fn runs_overlap(a: &[(f64, f64)], b: &[(f64, f64)]) -> bool {
    a.windows(2).filter(|w| (w[0].1 - w[1].1).abs() < EPS).any(|w| b.windows(2).filter(|v| (v[0].1 - v[1].1).abs() < EPS)
        .any(|v| (w[0].1 - v[0].1).abs() < EPS && w[0].0.max(w[1].0).min(v[0].0.max(v[1].0)) - w[0].0.min(w[1].0).max(v[0].0.min(v[1].0)) > 1.0))
}

#[cfg(test)]
fn false_crossings_for(yard: &Yard, shapes: &[Vec<(f64, f64)>]) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    for a in 0..shapes.len() {
        if shapes[a].is_empty() { continue; }
        for b in a + 1..shapes.len() {
            if shapes[b].is_empty() { continue; }
            let ea = &yard.edges[a]; let eb = &yard.edges[b];
            if [ea.a, ea.b].iter().any(|n| *n == eb.a || *n == eb.b) { continue; }
            if polylines_cross(&shapes[a], &shapes[b]) { result.push((a, b)); }
        }
    }
    result
}

fn polylines_cross(a: &[(f64, f64)], b: &[(f64, f64)]) -> bool {
    a.windows(2).any(|x| b.windows(2).any(|y| segments_cross(x[0], x[1], y[0], y[1])))
}

fn segments_cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    fn orient(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 { (b.0-a.0)*(c.1-a.1) - (b.1-a.1)*(c.0-a.0) }
    let (x, y, z, w) = (orient(a,b,c), orient(a,b,d), orient(c,d,a), orient(c,d,b));
    x * y < -EPS && z * w < -EPS
}

#[cfg(test)]
mod tests {
    use super::*;
    use cabin_sim::route::build_routes;

    fn layout() -> (Yard, Schematic) {
        let yard = Yard::parse(cabin_sim::NDLS).unwrap();
        let schematic = Schematic::new(&yard);
        (yard, schematic)
    }

    /// A platform's row is the row of its flat run, not of its end nodes: P4
    /// is the reverse leg of the point at node 91 where P3 is the normal, so
    /// P4 must leave that node at 45° (PLAN-schematic.md, S1 change of
    /// 2026-11-18). The flat run covers at least half the platform.
    #[test]
    fn every_platform_is_one_horizontal_run_in_order() {
        let (yard, s) = layout();
        let reverse: BTreeSet<(usize, usize)> = yard.points.iter().map(|p| (p.reverse, p.node)).collect();
        let mut platforms = Vec::new();
        for p in &yard.platforms {
            let (e, shape) = (p.edge, &s.shape[p.edge]);
            let span = (shape[0].0 - shape[shape.len() - 1].0).abs();
            let run = shape.windows(2).filter(|w| (w[0].1 - w[1].1).abs() < EPS)
                .max_by(|a, b| (a[0].0 - a[1].0).abs().total_cmp(&(b[0].0 - b[1].0).abs())).expect("platform has no flat run");
            assert!((run[0].0 - run[1].0).abs() >= span / 2.0, "P{} run is under half its length: {shape:?}", p.name);
            let edge = &yard.edges[e];
            for (i, w) in shape.windows(2).enumerate() {
                if (w[0].1 - w[1].1).abs() < EPS { continue; }
                let end = if i == 0 { Some(edge.a) } else if i == shape.len() - 2 { Some(edge.b) } else { None };
                assert!(end.is_some_and(|n| reverse.contains(&(e, n))), "P{} jogs away from a reverse-leg end: {shape:?}", p.name);
            }
            platforms.push((p.name.parse::<usize>().unwrap(), run[0].1));
        }
        platforms.sort_by_key(|p| p.0);
        for pair in platforms.windows(2) { assert!(pair[0].1 < pair[1].1, "platform rows are not ordered: {platforms:?}"); }
    }

    #[test]
    fn every_jog_is_45_degrees_or_flat() {
        let (_, s) = layout();
        for (e, shape) in s.shape.iter().enumerate().filter(|(_, p)| !p.is_empty()) {
            for w in shape.windows(2) {
                let (dx, dy) = ((w[1].0-w[0].0).abs(), (w[1].1-w[0].1).abs());
                assert!(dy < EPS || (dx-dy).abs() < EPS, "edge {e} has segment {w:?}");
            }
        }
    }

    #[test]
    fn edges_meet_at_their_nodes() {
        let (yard, s) = layout();
        for (e, edge) in yard.edges.iter().enumerate().filter(|(e, _)| !s.shape[*e].is_empty()) {
            let shape = &s.shape[e];
            assert_eq!(shape.first().copied(), s.node[edge.a]);
            assert_eq!(shape.last().copied(), s.node[edge.b]);
        }
    }

    #[test]
    fn every_route_draws_unbroken() {
        let (yard, s) = layout();
        for route in build_routes(&yard) {
            let mut chain = route.path.clone();
            chain.extend(route.overlap.iter().copied());
            for pair in chain.windows(2) {
                let a = &yard.edges[pair[0]]; let b = &yard.edges[pair[1]];
                let joins = [a.a, a.b].iter().any(|n| *n == b.a || *n == b.b);
                assert!(joins, "route {} has non-adjacent edges {} and {}", route.id, pair[0], pair[1]);
                let x = &s.shape[pair[0]]; let y = &s.shape[pair[1]];
                if x.is_empty() || y.is_empty() { continue; }
                let meet = [*x.first().unwrap(), *x.last().unwrap()].iter().any(|p| [*y.first().unwrap(), *y.last().unwrap()].iter().any(|q| dist(*p, *q) < EPS));
                assert!(meet, "route {} breaks between edges {} and {}", route.id, pair[0], pair[1]);
            }
        }
    }

    #[test]
    fn no_two_runs_share_a_row_where_they_overlap() {
        let (yard, s) = layout();
        let mut runs = Vec::new();
        let crossover_edges: BTreeSet<usize> = yard.points.iter().map(|p| p.reverse).collect();
        for (e, shape) in s.shape.iter().enumerate().filter(|(_, p)| !p.is_empty()) {
            if crossover_edges.contains(&e) { continue; }
            for w in shape.windows(2) {
                if (w[0].1-w[1].1).abs() < EPS {
                    runs.push((w[0].1, w[0].0.min(w[1].0), w[0].0.max(w[1].0), e));
                }
            }
        }
        for i in 0..runs.len() { for j in i+1..runs.len() {
            let a = runs[i]; let b = runs[j];
            if a.3 != b.3 && (a.0-b.0).abs() < EPS { assert!(a.2.min(b.2) - a.1.max(b.1) <= 1.0, "overlap on row {}: {a:?}, {b:?}", a.0); }
        }}
    }

    #[test]
    fn each_crossing_draws_as_an_x() {
        let (yard, s) = layout();
        for crossing in &yard.crossings {
            let Some(at) = s.node[crossing.node] else { continue };
            for &(a, b) in crossing.pairs.iter().take(2) {
                for e in [a, b] {
                    if s.shape[e].is_empty() { continue; }
                    assert!(s.shape[e].iter().any(|p| dist(*p, at) < EPS), "crossing edge {e} misses node {}", crossing.node);
                }
            }
        }
    }

    #[test]
    fn false_crossings_are_none() {
        let (yard, s) = layout();
        let pairs = false_crossings_for(&yard, &s.shape);
        eprintln!("schematic: false_crossings={:?}", pairs.iter().map(|&(a,b)| (yard.edges[a].way, yard.edges[b].way)).collect::<Vec<_>>());
        assert!(pairs.is_empty(), "false crossings (edge ids and OSM ways): {:?}", pairs.iter().map(|&(a,b)| (a, yard.edges[a].way, b, yard.edges[b].way)).collect::<Vec<_>>());
    }
}

#[cfg(test)]
fn dist(a: (f64, f64), b: (f64, f64)) -> f64 { (a.0-b.0).hypot(a.1-b.1) }

