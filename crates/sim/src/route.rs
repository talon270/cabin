//! CABIN · ROUTES
//!   · build_routes: every route a signal can set, shortest path per
//!     (entry, exit)
//!   · Route: its path, the points it sets and their lie, its overlap, and
//!     `holds`, every resource it locks
//!
//! A route runs from its entry signal to the first signal ahead that faces
//! the same way, or out through a portal. It never passes a signal for its
//! own direction, so a train always meets the exit signal it was routed to.
//! Only running track is routed (plan, "What is worked"). Built at load and
//! never stored: the yard file holds track and signals, not routes.
//!
//! `holds` is what `Interlocking::request` (B4) compares: two routes whose
//! holds share a resource conflict. A point is one resource whatever its
//! lie, so two routes over one point conflict even when both want it the
//! same way; that is cautious, never unsafe. Fouled legs need train
//! positions, so B4's occupancy check covers them, not `holds`.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use crate::consts::OVERLAP_M;
use crate::yard::{Graph, Junction, Kind, Yard};

pub type SignalId = usize;
pub type PlatformId = usize;
pub type PortalId = usize;
pub type RouteId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Exit {
    /// Stop at this Starter, berthed at its platform.
    Berth { platform: PlatformId, starter: SignalId },
    /// Leave the extract here.
    Portal(PortalId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Res {
    Edge(usize),
    Point(usize),
    Crossing(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lie {
    Normal,
    Reverse,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub id: RouteId,
    pub entry: SignalId,
    pub exit: Exit,
    /// Edges from the entry signal's edge to the exit's, in running order.
    pub path: Vec<usize>,
    /// Every point the route sets, on its path and in its overlap, in order.
    pub points: Vec<(usize, Lie)>,
    /// Edges within OVERLAP_M beyond a Berth's exit signal. Empty for a
    /// portal exit: the line beyond is the block section (B4, C3-148).
    pub overlap: Vec<usize>,
    pub holds: BTreeSet<Res>,
    /// Metres from the entry signal to the exit signal or portal.
    pub len_m: f64,
}

/// What passing from `from` into `to` over node `n` sets or crosses.
fn passing(yard: &Yard, g: &Graph, n: usize, from: usize, to: usize, points: &mut Vec<(usize, Lie)>, holds: &mut BTreeSet<Res>) {
    match g.at[n] {
        Junction::Point(k) => {
            let p = &yard.points[k];
            let leg = if from == p.stem { to } else { from };
            points.push((k, if leg == p.reverse { Lie::Reverse } else { Lie::Normal }));
            holds.insert(Res::Point(k));
        }
        Junction::Crossing(k) => {
            holds.insert(Res::Crossing(k));
        }
        _ => {}
    }
}

/// The overlap beyond a signal at (`edge`, `offset`, `forward`): OVERLAP_M
/// along the line, over any kind of track. Facing points are taken normal,
/// where points rest (C3-164 "set for the straight"), and a slip is taken
/// straight, its first pair; trailing points are set for the leg it comes
/// from. It stops early at an end or a portal.
fn overlap(yard: &Yard, g: &Graph, edge: usize, offset: f64, forward: bool, points: &mut Vec<(usize, Lie)>, holds: &mut BTreeSet<Res>) -> Vec<usize> {
    let len = yard.edges[edge].len;
    let mut left = OVERLAP_M - if forward { len - offset } else { offset };
    let (mut e, mut f) = (edge, forward);
    let mut out = Vec::new();
    while left > 0.0 && out.len() < yard.edges.len() {
        let n = yard.head(e, f);
        let next = match g.at[n] {
            Junction::Point(k) if e == yard.points[k].stem => {
                let normal = yard.points[k].normal;
                Some((normal, yard.edges[normal].a == n))
            }
            Junction::Other => None,
            _ => g.next(yard, e, f).first().copied(),
        };
        let Some((ne, nf)) = next else { break };
        passing(yard, g, n, e, ne, points, holds);
        out.push(ne);
        holds.insert(Res::Edge(ne));
        left -= yard.edges[ne].len;
        (e, f) = (ne, nf);
    }
    out
}

/// Distances are kept in whole centimetres so the search order, and so
/// every route, is the same on every machine.
fn cm(m: f64) -> i64 {
    (m * 100.0).round() as i64
}

pub fn build_routes(yard: &Yard) -> Vec<Route> {
    let g = yard.graph();
    // Signals per (edge, direction), ordered along the direction of travel.
    let mut on: BTreeMap<(usize, bool), Vec<usize>> = BTreeMap::new();
    for (k, s) in yard.signals.iter().enumerate() {
        on.entry((s.edge, s.forward)).or_default().push(k);
    }
    for ((_, f), list) in on.iter_mut() {
        list.sort_by(|&a, &b| {
            let (x, y) = (yard.signals[a].offset, yard.signals[b].offset);
            if *f { x.total_cmp(&y) } else { y.total_cmp(&x) }
        });
    }
    // Distance along the direction of travel from the edge's tail.
    let along = |edge: usize, forward: bool, offset: f64| if forward { offset } else { yard.edges[edge].len - offset };
    let portal_at: BTreeMap<(usize, usize), usize> = yard.portals.iter().enumerate().map(|(k, p)| ((p.edge, p.node), k)).collect();

    let mut routes = Vec::new();
    for (entry, s) in yard.signals.iter().enumerate() {
        if yard.edges[s.edge].kind != Kind::Running {
            continue;
        }
        let start = (s.edge, s.forward);
        // Distance at each state is measured at the tail of its edge, so the
        // entry's own edge starts negative.
        let mut dist: BTreeMap<(usize, bool), i64> = BTreeMap::new();
        let mut parent: BTreeMap<(usize, bool), (usize, bool)> = BTreeMap::new();
        let mut best: BTreeMap<Exit, (i64, (usize, bool))> = BTreeMap::new();
        let mut heap = BinaryHeap::new();
        dist.insert(start, -cm(along(s.edge, s.forward, s.offset)));
        heap.push(Reverse((dist[&start], start)));
        while let Some(Reverse((d, st))) = heap.pop() {
            if dist.get(&st).is_some_and(|&x| x < d) {
                continue;
            }
            let (e, f) = st;
            // The first signal for this direction ahead on this edge ends
            // the route; on the entry's own edge, only one beyond the entry.
            let from = if st == start { along(e, f, s.offset) } else { f64::NEG_INFINITY };
            let ahead = on.get(&st).and_then(|l| l.iter().copied().find(|&k| along(e, f, yard.signals[k].offset) > from + 1e-9));
            if let Some(x) = ahead {
                if let (Some(platform), true) = (yard.signals[x].platform, x != entry) {
                    let exit = Exit::Berth { platform, starter: x };
                    let total = d + cm(along(e, f, yard.signals[x].offset));
                    if best.get(&exit).is_none_or(|b| total < b.0) {
                        best.insert(exit, (total, st));
                    }
                }
                continue;
            }
            let head = yard.head(e, f);
            if let Some(&p) = portal_at.get(&(e, head)) {
                let total = d + cm(yard.edges[e].len);
                if best.get(&Exit::Portal(p)).is_none_or(|b| total < b.0) {
                    best.insert(Exit::Portal(p), (total, st));
                }
                continue;
            }
            for nx in g.next(yard, e, f) {
                if yard.edges[nx.0].kind != Kind::Running {
                    continue;
                }
                let nd = d + cm(yard.edges[e].len);
                if dist.get(&nx).is_none_or(|&old| nd < old) {
                    dist.insert(nx, nd);
                    parent.insert(nx, st);
                    heap.push(Reverse((nd, nx)));
                }
            }
        }
        for (exit, (total, last)) in best {
            let mut chain = vec![last];
            while let Some(&p) = parent.get(chain.last().unwrap()) {
                chain.push(p);
            }
            chain.reverse();
            let path: Vec<usize> = chain.iter().map(|st| st.0).collect();
            let mut points = Vec::new();
            let mut holds: BTreeSet<Res> = path.iter().map(|&e| Res::Edge(e)).collect();
            for w in chain.windows(2) {
                passing(yard, &g, yard.head(w[0].0, w[0].1), w[0].0, w[1].0, &mut points, &mut holds);
            }
            let overlap = match exit {
                Exit::Berth { starter, .. } => {
                    let x = &yard.signals[starter];
                    overlap(yard, &g, x.edge, x.offset, x.forward, &mut points, &mut holds)
                }
                Exit::Portal(_) => Vec::new(),
            };
            routes.push(Route { id: routes.len(), entry, exit, path, points, overlap, holds, len_m: total as f64 / 100.0 });
        }
    }
    routes
}
