//! CABIN · INTERLOCKING
//!   · request: the one function that locks a route; every clearance passes it
//!   · cancel, release_behind: the only ways a lock goes away
//!   · aspect, lie, held: derived from the locks on every call, never stored
//!
//! Guard 1 of the plan: no signal clears onto a conflicting route, an
//! occupied track or unlocked points (C3-139 "facing points set and
//! locked"). The lock table is private; `request` is its only inserter.
//! Trains take movement authority from a live lock and nothing else.
//!
//! `request` checks, in this order, and the first failure is the refusal:
//!   1. the route exists
//!   2. no train stands across the entry signal (C3-134 "until the whole
//!      train has passed")
//!   3. no live lock holds any of its resources (C3-154 "for one only")
//!   4. no train on its path, or within FOULING_M of a junction on it
//!   5. no train on its overlap (C3-143 "120 m"), the same two ways
//!   6. for a portal exit, no train is still coming in by it, and the
//!      block beyond is free (C3-148 "Line Clear")
//!
//! A berthed train's route lets go of everything but the junctions the
//! body still stands on, so a departure can be set behind it; occupancy,
//! checked by every request, protects the track the train stands on.

use std::collections::{BTreeMap, BTreeSet};

use crate::consts::{APPROACH_RELEASE_S, BLOCK_HEADWAY_S, FOULING_M, TICK_HZ};
use crate::route::{Exit, Lie, PortalId, Res, Route, RouteId, SignalId};
use crate::train::{Body, Occupancy, TrainId, EPS};
use crate::yard::{Junction, Yard};

/// A body within this of a junction's node stands on the junction itself.
const ON_NODE_M: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aspect {
    Red,
    Yellow,
    DoubleYellow,
    Green,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NoSuchRoute,
    Conflict { with: RouteId, at: Res },
    Occupied { at: Res, by: TrainId },
    OverlapOccupied { at: Res, by: TrainId },
    NoLineClear { portal: PortalId, wait_s: u32 },
    TrainPastSignal { train: TrainId },
    ApproachLocked { release_in_s: u32 },
}

/// A route's track in running order, path then overlap, and the node and
/// junction (if any) between each pair of consecutive edges.
#[derive(Clone, Debug, PartialEq)]
struct Run {
    edges: Vec<usize>,
    /// `nodes[i]` lies between `edges[i]` and `edges[i + 1]`.
    nodes: Vec<(usize, Option<Res>)>,
    path_len: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct Lock {
    held: BTreeSet<Res>,
    /// The train that has taken the route: the one approaching its entry.
    train: Option<TrainId>,
    /// The train is past the entry signal.
    entered: bool,
    /// Put back with a train on the approach: the tick the locks go.
    cancel_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Interlocking {
    routes: Vec<Route>,
    runs: Vec<Run>,
    locks: BTreeMap<RouteId, Lock>,
    now: u64,
    /// The tick the last train left by each portal.
    last_exit: BTreeMap<PortalId, u64>,
}

fn junction(g: &crate::yard::Graph, n: usize) -> Option<Res> {
    match g.at[n] {
        Junction::Point(k) => Some(Res::Point(k)),
        Junction::Crossing(k) => Some(Res::Crossing(k)),
        _ => None,
    }
}

/// Whole seconds until `tick`, rounded up.
fn secs_until(now: u64, tick: u64) -> u32 {
    (tick.saturating_sub(now)).div_ceil(TICK_HZ as u64) as u32
}

/// Metres along signal `s`'s direction from its edge's tail.
fn signal_along(yard: &Yard, s: SignalId) -> f64 {
    let sg = &yard.signals[s];
    if sg.forward { sg.offset } else { yard.edges[sg.edge].len - sg.offset }
}

impl Interlocking {
    pub fn new(yard: &Yard, routes: Vec<Route>) -> Interlocking {
        let g = yard.graph();
        let runs = routes
            .iter()
            .map(|r| {
                let edges: Vec<usize> = r.path.iter().chain(&r.overlap).copied().collect();
                let nodes = edges
                    .windows(2)
                    .map(|w| {
                        let (x, z) = (&yard.edges[w[0]], &yard.edges[w[1]]);
                        let n = if x.a == z.a || x.a == z.b { x.a } else { x.b };
                        (n, junction(&g, n))
                    })
                    .collect();
                Run { edges, nodes, path_len: r.path.len() }
            })
            .collect();
        Interlocking { routes, runs, locks: BTreeMap::new(), now: 0, last_exit: BTreeMap::new() }
    }

    pub fn routes(&self) -> &[Route] {
        &self.routes
    }

    pub fn set_now(&mut self, tick: u64) {
        self.now = tick;
    }

    /// A train has left by `portal`; the block beyond is now occupied.
    pub fn train_left(&mut self, portal: PortalId) {
        self.last_exit.insert(portal, self.now);
    }

    /// The block beyond `portal` is free, or the seconds until it is.
    pub fn line_clear(&self, portal: PortalId) -> Result<(), Refusal> {
        let free_at = self.last_exit.get(&portal).map_or(0, |&t| t + (BLOCK_HEADWAY_S * TICK_HZ) as u64);
        if self.now < free_at { Err(Refusal::NoLineClear { portal, wait_s: secs_until(self.now, free_at) }) } else { Ok(()) }
    }

    /// The live lock that holds `r`, as a refusal.
    pub fn holder(&self, r: Res) -> Option<Refusal> {
        self.locks.iter().find(|(_, l)| l.held.contains(&r)).map(|(&id, l)| match l.cancel_at {
            Some(t) => Refusal::ApproachLocked { release_in_s: secs_until(self.now, t) },
            None => Refusal::Conflict { with: id, at: r },
        })
    }

    pub fn request(&mut self, yard: &Yard, occ: &Occupancy, entry: SignalId, exit: Exit) -> Result<RouteId, Refusal> {
        let id = self.routes.iter().position(|r| r.entry == entry && r.exit == exit).ok_or(Refusal::NoSuchRoute)?;
        let (r, run) = (&self.routes[id], &self.runs[id]);

        let sg = &yard.signals[entry];
        if let Some(t) = occ.on(sg.edge, sg.offset - EPS, sg.offset + EPS) {
            return Err(Refusal::TrainPastSignal { train: t });
        }

        for &res in &r.holds {
            if let Some(refusal) = self.holder(res) {
                return Err(refusal);
            }
        }

        // Track first, then the junctions between, path before overlap.
        let along = signal_along(yard, entry);
        for (i, &e) in run.edges.iter().enumerate() {
            let len = yard.edges[e].len;
            // On the entry's own edge only the part ahead of the signal
            // counts: the train waiting at it stands behind.
            let (lo, hi) = match (i, sg.forward) {
                (0, true) => (along, len),
                (0, false) => (0.0, len - along),
                _ => (0.0, len),
            };
            if let Some(by) = occ.on(e, lo, hi) {
                let at = Res::Edge(e);
                return Err(if i < run.path_len { Refusal::Occupied { at, by } } else { Refusal::OverlapOccupied { at, by } });
            }
        }
        for (i, &(n, res)) in run.nodes.iter().enumerate() {
            let Some(at) = res else { continue };
            if let Some(by) = occ.near(yard, n, FOULING_M, &run.edges) {
                // nodes[i] closes edges[i]: still on the path while edges[i + 1] is.
                return Err(if i + 1 < run.path_len { Refusal::Occupied { at, by } } else { Refusal::OverlapOccupied { at, by } });
            }
        }

        if let Exit::Portal(p) = exit {
            if let Some(by) = occ.entering(p) {
                return Err(Refusal::Occupied { at: Res::Edge(yard.portals[p].edge), by });
            }
            self.line_clear(p)?;
        }

        self.locks.insert(id, Lock { held: r.holds.clone(), train: None, entered: false, cancel_at: None });
        Ok(id)
    }

    /// Put a route's signal back. With no train on the approach the locks
    /// go at once; with one, they stay APPROACH_RELEASE_S, because the
    /// driver may already be past the point of stopping.
    pub fn cancel(&mut self, yard: &Yard, occ: &Occupancy, route: RouteId) -> Result<(), Refusal> {
        let Some(l) = self.locks.get(&route) else { return Err(Refusal::NoSuchRoute) };
        let sg = &yard.signals[self.routes[route].entry];
        if l.entered || occ.on(sg.edge, sg.offset - EPS, sg.offset + EPS).is_some() {
            let train = l.train.or_else(|| occ.on(sg.edge, sg.offset - EPS, sg.offset + EPS)).unwrap_or(0);
            return Err(Refusal::TrainPastSignal { train });
        }
        if let Some(t) = l.cancel_at {
            return Err(Refusal::ApproachLocked { release_in_s: secs_until(self.now, t) });
        }
        if l.train.is_some() {
            let at = self.now + (APPROACH_RELEASE_S * TICK_HZ) as u64;
            self.locks.get_mut(&route).unwrap().cancel_at = Some(at);
        } else {
            self.locks.remove(&route);
        }
        Ok(())
    }

    /// The set route a train at `signal` may take, if its signal is off.
    pub fn takeable(&self, signal: SignalId) -> Option<RouteId> {
        self.locks
            .iter()
            .find(|&(&id, l)| self.routes[id].entry == signal && !l.entered && l.cancel_at.is_none() && l.train.is_none())
            .map(|(&id, _)| id)
    }

    pub fn take(&mut self, route: RouteId, train: TrainId) {
        if let Some(l) = self.locks.get_mut(&route) {
            l.train = Some(train);
        }
    }

    /// A train stopped short of a route put back: it no longer holds it.
    pub fn untake(&mut self, route: RouteId) {
        if let Some(l) = self.locks.get_mut(&route) {
            l.train = None;
        }
    }

    /// Let go of what each train has passed. Called after every tick's
    /// movement, so a lock never outlives the track it protects by more
    /// than one tick.
    pub fn release_behind(&mut self, yard: &Yard, occ: &Occupancy) {
        let now = self.now;
        self.locks.retain(|_, l| l.cancel_at.is_none_or(|t| l.entered || now < t));
        for (&id, l) in self.locks.iter_mut() {
            let Some(t) = l.train else { continue };
            let (r, run) = (&self.routes[id], &self.runs[id]);
            let Some(b) = occ.bodies.get(&t) else {
                // Gone by its portal.
                if l.entered {
                    l.held.clear();
                }
                continue;
            };
            if !l.entered {
                let sg = &yard.signals[r.entry];
                let past_signal = b.spans.iter().any(|s| s.edge == sg.edge && if sg.forward { s.hi > sg.offset + EPS } else { s.lo < sg.offset - EPS });
                if past_signal || b.spans.iter().any(|s| r.path[1..].contains(&s.edge)) {
                    l.entered = true;
                    l.cancel_at = None;
                }
            }
            if !l.entered {
                continue;
            }
            let berthed = matches!(r.exit, Exit::Berth { .. }) && b.stopped && b.head.0 == *r.path.last().unwrap();
            // The first edge of the run the body is still on.
            let j = if berthed { run.edges.len() } else { run.edges.iter().position(|e| b.spans.iter().any(|s| s.edge == *e)).unwrap_or(run.edges.len()) };
            for &e in &run.edges[..j] {
                l.held.remove(&Res::Edge(e));
            }
            if berthed {
                // The track it stands on is the occupancy check's; only a
                // point or crossing under the body stays locked.
                for &e in &run.edges {
                    l.held.remove(&Res::Edge(e));
                }
            }
            let clear_m = if berthed { ON_NODE_M } else { FOULING_M };
            for &(n, res) in &run.nodes[..j.min(run.nodes.len())] {
                if let Some(res) = res
                    && !body_near(yard, b, n, clear_m, &[])
                {
                    l.held.remove(&res);
                }
            }
        }
        self.locks.retain(|_, l| !l.held.is_empty());
    }

    pub fn aspect(&self, signal: SignalId) -> Aspect {
        let live = self.locks.iter().find(|&(&id, l)| self.routes[id].entry == signal && !l.entered && l.cancel_at.is_none());
        let Some((&id, _)) = live else { return Aspect::Red };
        match self.routes[id].exit {
            // The Starter's off aspect is not settled by the ledger
            // (`rails/README.md`); green onto a clear block is assumed.
            Exit::Portal(_) => Aspect::Green,
            Exit::Berth { starter, .. } => match self.aspect(starter) {
                Aspect::Red => Aspect::Yellow,
                Aspect::Yellow => Aspect::DoubleYellow,
                _ => Aspect::Green,
            },
        }
    }

    /// The lie of point `k`: its route's, or normal when no route holds it
    /// (C3-164 "set for the straight").
    pub fn lie(&self, k: usize) -> Lie {
        for (&id, l) in &self.locks {
            if l.held.contains(&Res::Point(k))
                && let Some(&(_, lie)) = self.routes[id].points.iter().find(|p| p.0 == k)
            {
                return lie;
            }
        }
        Lie::Normal
    }

    /// Whether any live route still holds `r`.
    pub fn held(&self, r: Res) -> bool {
        self.locks.values().any(|l| l.held.contains(&r))
    }

    /// The route a train has taken and not yet passed the entry of.
    pub fn taken_by(&self, train: TrainId) -> Vec<RouteId> {
        self.locks.iter().filter(|(_, l)| l.train == Some(train) && !l.entered).map(|(&id, _)| id).collect()
    }
}

/// `b` comes within `m` metres of `node` on an incident edge not in `except`.
pub fn body_near(yard: &Yard, b: &Body, node: usize, m: f64, except: &[usize]) -> bool {
    b.spans.iter().any(|s| {
        let e = &yard.edges[s.edge];
        if except.contains(&s.edge) {
            return false;
        }
        (e.a == node && s.lo < m - EPS) || (e.b == node && s.hi > e.len - m + EPS)
    })
}
