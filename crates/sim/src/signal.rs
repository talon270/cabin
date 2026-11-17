//! CABIN · SIGNALS
//!   · generate: a Home on each approach line, a Starter at each platform
//!     end a train can leave by
//!   · side: which end of the station a node lies at, N, S, E or W
//!
//! No public NDLS signalling plan was found (plan A3), and OpenStreetMap
//! maps 2 signals for the station, so every position here comes from a rule
//! and an authored distance. `cabin-osm` runs this, applies the overrides
//! file on top, and writes the result into `data/ndls.yard`. Names: a Home
//! is `H` and its portal (`HN1`); a Starter is `ST`, its platform and the
//! side it faces (`ST5S`).

use crate::consts::{HOME_SETBACK_M, STARTER_SETBACK_M};
use crate::yard::{Graph, Junction, Kind, Origin, Signal, SignalKind, Yard};

/// From (edge, forward), run on through nodes that offer no choice to the
/// next junction: a point, a crossing or an end. Returns the last edge, its
/// direction, and the node it meets. A way boundary is not a connection, so
/// a signal that must stand before the first connection looks past it. The
/// end of running track counts as a junction: routes are set over running
/// track only, and a Starter placed on the siding beyond (five were, at
/// first) is a signal no route can reach.
fn run_to_junction(yard: &Yard, g: &Graph, mut edge: usize, mut forward: bool) -> (usize, bool, usize) {
    for _ in 0..yard.edges.len() {
        let n = yard.head(edge, forward);
        if g.at[n] != Junction::Through {
            return (edge, forward, n);
        }
        match g.next(yard, edge, forward).first() {
            Some(&(e, f)) if yard.edges[e].kind == Kind::Running => (edge, forward) = (e, f),
            _ => return (edge, forward, n),
        }
    }
    (edge, forward, yard.head(edge, forward))
}

/// The offset on `edge` that lies `m` metres before its head, clamped to
/// the edge.
fn before_head(yard: &Yard, edge: usize, forward: bool, m: f64) -> f64 {
    let len = yard.edges[edge].len;
    if forward { (len - m).max(0.0) } else { m.min(len) }
}

/// The station end `node` lies at, seen from `(x, y)`: N, S, E or W by the
/// larger of the two offsets, the rule `cabin-osm` names portals by.
pub fn side(yard: &Yard, from: (f64, f64), node: usize) -> char {
    let (dx, dy) = (yard.nodes[node].x - from.0, yard.nodes[node].y - from.1);
    if dy.abs() >= dx.abs() {
        if dy > 0.0 { 'N' } else { 'S' }
    } else if dx > 0.0 {
        'E'
    } else {
        'W'
    }
}

pub fn generate(yard: &Yard) -> Vec<Signal> {
    let g = yard.graph();
    let portal_nodes: Vec<usize> = yard.portals.iter().map(|p| p.node).collect();
    let mut out = Vec::new();
    for p in &yard.portals {
        // A train comes in from the portal, so the Home faces inward.
        let inward = yard.edges[p.edge].a == p.node;
        let (edge, forward, _) = run_to_junction(yard, &g, p.edge, inward);
        out.push(Signal {
            name: format!("H{}", p.name),
            kind: SignalKind::Home,
            edge,
            offset: before_head(yard, edge, forward, HOME_SETBACK_M),
            forward,
            origin: Origin::Generated,
            platform: None,
        });
    }
    let mut done: Vec<usize> = Vec::new();
    for (k, pl) in yard.platforms.iter().enumerate() {
        // Two platform faces can serve one track; it gets one pair.
        if done.contains(&pl.edge) {
            continue;
        }
        done.push(pl.edge);
        let e = &yard.edges[pl.edge];
        let mid = e.poly[e.poly.len() / 2];
        for forward in [true, false] {
            let (edge, f, n) = run_to_junction(yard, &g, pl.edge, forward);
            // A buffer stop ahead: no train leaves this way, so no Starter.
            if g.at[n] == Junction::End && !portal_nodes.contains(&n) {
                continue;
            }
            out.push(Signal {
                name: format!("ST{}{}", pl.name, side(yard, mid, n)),
                kind: SignalKind::Starter,
                edge,
                offset: before_head(yard, edge, f, STARTER_SETBACK_M),
                forward: f,
                origin: Origin::Generated,
                platform: Some(k),
            });
        }
    }
    out
}
