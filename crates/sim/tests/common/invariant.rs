//! The invariant checker. It reads train bodies, point lies and the yard,
//! and never asks the interlocking whether a move was allowed: a simulator
//! checked only against its own code proves nothing (`rails/README.md`).
//! One read of the locks remains, rule 3, because "a moving train holds a
//! lock ahead" cannot be said without them.
//!
//! After every tick:
//!   1. no two bodies share an edge
//!   2. no two bodies stand within FOULING_M of one junction on different
//!      edges (a body across the junction counts on both)
//!   3. a moving train's head edge is locked, unless the head is still in
//!      the approach block between a portal and its Home
//!   4. no point changes its lie while a body stands on it (within 1 m of
//!      its node). Standing near it is rule 2's business: a point moving
//!      beside a train harms no one unless another train comes, and rule 2
//!      fails that first.

use std::collections::{BTreeMap, BTreeSet};

use cabin_sim::consts::FOULING_M;
use cabin_sim::route::{Lie, Res};
use cabin_sim::train::Body;
use cabin_sim::yard::{SignalKind, Yard};
use cabin_sim::World;

/// Stored positions carry one decimal, so a train stopped at a Home 30 m
/// out may read 29.95 m. A millimetre is no physical distance.
const EPS: f64 = 1e-3;

pub struct Checker {
    lies: Vec<Lie>,
    /// Per portal, the edges from the box edge to its Home, and the Home.
    blocks: Vec<(Vec<usize>, usize)>,
    pub ticks: u64,
}

/// Edges at `node`, and which end of each meets it (true: the `a` end).
fn incident(y: &Yard, node: usize) -> Vec<(usize, bool)> {
    let mut v = Vec::new();
    for (k, e) in y.edges.iter().enumerate() {
        if e.a == node {
            v.push((k, true));
        }
        if e.b == node {
            v.push((k, false));
        }
    }
    v
}

/// Edges at `node` on which the body comes within `m` of it.
fn near(y: &Yard, b: &Body, node: usize, m: f64) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    for (e, at_a) in incident(y, node) {
        let len = y.edges[e].len;
        for s in b.spans.iter().filter(|s| s.edge == e) {
            let close = if at_a { s.lo < m - EPS } else { s.hi > len - m + EPS };
            if close {
                out.insert(e);
            }
        }
    }
    out
}

impl Checker {
    pub fn new(w: &World) -> Checker {
        let y = &w.yard;
        let g = y.graph();
        let mut blocks = Vec::new();
        for p in &y.portals {
            let home = y.signals.iter().position(|s| s.kind == SignalKind::Home && s.name[1..] == p.name).expect("a Home per portal");
            let (mut e, mut f) = (p.edge, y.edges[p.edge].a == p.node);
            let mut edges = vec![e];
            while e != y.signals[home].edge {
                let Some(&(ne, nf)) = g.next(y, e, f).first() else { break };
                (e, f) = (ne, nf);
                edges.push(e);
            }
            blocks.push((edges, home));
        }
        Checker { lies: (0..y.points.len()).map(|k| w.lie(k)).collect(), blocks, ticks: 0 }
    }

    fn in_block(&self, y: &Yard, b: &Body) -> bool {
        let (he, at) = b.head;
        self.blocks.iter().any(|(edges, home)| {
            let h = &y.signals[*home];
            edges.contains(&he) && (he != h.edge || if h.forward { at <= h.offset + EPS } else { at >= h.offset - EPS })
        })
    }

    pub fn check(&mut self, w: &World) {
        self.ticks += 1;
        let y = &w.yard;
        let occ = w.occupancy();
        let bodies: Vec<(usize, &Body)> = occ.bodies.iter().map(|(&t, b)| (t, b)).collect();
        let fail = |what: String| panic!("tick {}: {what}", w.tick);

        // 1. Sections.
        let mut on: BTreeMap<usize, usize> = BTreeMap::new();
        for &(t, b) in &bodies {
            for s in &b.spans {
                if let Some(&o) = on.get(&s.edge)
                    && o != t
                {
                    fail(format!("trains {o} and {t} share edge {}", s.edge));
                }
                on.insert(s.edge, t);
            }
        }

        // 2. Fouling at every point and crossing.
        let junctions: Vec<usize> = y.points.iter().map(|p| p.node).chain(y.crossings.iter().map(|c| c.node)).collect();
        for &n in &junctions {
            let close: Vec<(usize, BTreeSet<usize>)> = bodies.iter().map(|&(t, b)| (t, near(y, b, n, FOULING_M))).filter(|(_, s)| !s.is_empty()).collect();
            if close.len() > 1 {
                fail(format!("trains {:?} all stand within {FOULING_M} m of junction node {n}", close));
            }
        }

        // 3. Movement authority comes from a lock.
        for &(t, b) in &bodies {
            if !b.stopped && !self.in_block(y, b) && !w.lock.held(Res::Edge(b.head.0)) {
                fail(format!("train {t} moves on edge {} with no lock on it", b.head.0));
            }
        }

        // 4. No point moves under a train.
        for (k, p) in y.points.iter().enumerate() {
            let now = w.lie(k);
            if now != self.lies[k] && bodies.iter().any(|&(_, b)| !near(y, b, p.node, 1.0).is_empty()) {
                fail(format!("point {k} moved {:?} -> {now:?} with a train on it", self.lies[k]));
            }
            self.lies[k] = now;
        }
    }
}
