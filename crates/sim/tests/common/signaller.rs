//! The auto-signaller (B5): a test-only controller that plays the cabin.
//! Every RETRY_S it looks at each waiting train and sets the first route,
//! in route order, that the interlocking accepts:
//!   · a train at a Home: a route into a berth whose Starter has a
//!     departure (a berth with none would strand the train: no train
//!     reverses in this model)
//!   · a train stopped at a Starter after its dwell: a route out by any
//!     portal
//!
//! It counts what the plan asks B5 to print: decisions per game hour, each
//! refusal by kind, and how often a refused route's path was blocked while
//! another path to the same exit was free.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use cabin_sim::consts::{FOULING_M, TICK_HZ};
use cabin_sim::interlock::Refusal;
use cabin_sim::route::{Exit, Res, Route, RouteId, SignalId};
use cabin_sim::train::{Occupancy, TrainId, EPS};
use cabin_sim::yard::{Graph, Junction, Kind, SignalKind, Yard};
use cabin_sim::{Action, World};

/// Game seconds between two looks at the waiting trains. A signalman does
/// not retry every half second either.
pub const RETRY_S: u64 = 10;

pub struct Signaller {
    g: Graph,
    /// Per signal, the routes the signaller may set from it, in route order.
    choices: BTreeMap<SignalId, Vec<RouteId>>,
    /// Per (edge, direction), the signals on it.
    signals_on: BTreeMap<(usize, bool), Vec<SignalId>>,
    /// Per train, its dwell in seconds, and the tick it ends once berthed.
    pub dwell_s: BTreeMap<TrainId, u64>,
    dwell_until: BTreeMap<TrainId, u64>,
    /// Per (train, signal), the tick the train began to wait there.
    waiting_since: BTreeMap<(TrainId, SignalId), u64>,
    /// Successful route settings per game hour.
    pub per_hour: Vec<u64>,
    pub refusals: BTreeMap<&'static str, u64>,
    /// `Occupied` refusals at a point or crossing by a train standing at a
    /// Starter: the plan's B4 INCONSISTENCY, Starters inside FOULING_M.
    pub fouled_by_berthed: u64,
    /// Refused requests whose own path was blocked, and of those, how many
    /// had another free path to the same exit.
    pub path_blocked: u64,
    pub alt_free: u64,
    /// Longest wait in seconds at a Home and at a Starter after the dwell.
    pub max_wait_home_s: u64,
    pub max_wait_starter_s: u64,
    /// The Starter of that longest wait.
    pub max_wait_starter: Option<SignalId>,
    /// The last tick any route was set.
    pub last_decision: u64,
}

fn kind(r: &Refusal) -> &'static str {
    match r {
        Refusal::NoSuchRoute => "NoSuchRoute",
        Refusal::Conflict { .. } => "Conflict",
        Refusal::Occupied { .. } => "Occupied",
        Refusal::OverlapOccupied { .. } => "OverlapOccupied",
        Refusal::NoLineClear { .. } => "NoLineClear",
        Refusal::TrainPastSignal { .. } => "TrainPastSignal",
        Refusal::ApproachLocked { .. } => "ApproachLocked",
    }
}

fn junction(g: &Graph, n: usize) -> Option<Res> {
    match g.at[n] {
        Junction::Point(k) => Some(Res::Point(k)),
        Junction::Crossing(k) => Some(Res::Crossing(k)),
        _ => None,
    }
}

impl Signaller {
    pub fn new(w: &World) -> Signaller {
        let y = &w.yard;
        let routes = w.lock.routes();
        let departs = |s: SignalId| routes.iter().any(|d| d.entry == s && matches!(d.exit, Exit::Portal(_)));
        let mut choices: BTreeMap<SignalId, Vec<RouteId>> = BTreeMap::new();
        for r in routes {
            let ok = match (y.signals[r.entry].kind, r.exit) {
                (SignalKind::Home, Exit::Berth { starter, .. }) => departs(starter),
                (SignalKind::Starter, Exit::Portal(_)) => true,
                _ => false,
            };
            if ok {
                choices.entry(r.entry).or_default().push(r.id);
            }
        }
        let mut signals_on: BTreeMap<(usize, bool), Vec<SignalId>> = BTreeMap::new();
        for (k, s) in y.signals.iter().enumerate() {
            signals_on.entry((s.edge, s.forward)).or_default().push(k);
        }
        Signaller {
            g: y.graph(),
            choices,
            signals_on,
            dwell_s: BTreeMap::new(),
            dwell_until: BTreeMap::new(),
            waiting_since: BTreeMap::new(),
            per_hour: Vec::new(),
            refusals: BTreeMap::new(),
            fouled_by_berthed: 0,
            path_blocked: 0,
            alt_free: 0,
            max_wait_home_s: 0,
            max_wait_starter_s: 0,
            max_wait_starter: None,
            last_decision: 0,
        }
    }

    pub fn decisions(&self) -> u64 {
        self.per_hour.iter().sum()
    }

    fn decided(&mut self, now: u64) {
        let hour = (now / (3600 * TICK_HZ as u64)) as usize;
        if self.per_hour.len() <= hour {
            self.per_hour.resize(hour + 1, 0);
        }
        self.per_hour[hour] += 1;
        self.last_decision = now;
    }

    /// Accept a train offered at `portal` only if a berth route from its
    /// Home can be set the moment it arrives, tried on a copy of the world;
    /// on success the copy, with the train in and its route set, becomes
    /// the world. A signalman gives Line Clear to the station in rear only
    /// for a train he can receive. Without this, the B5 soak gridlocked on
    /// 2 of 3 seeds: arrivals waited at all 12 Homes, and every departure
    /// needed a portal track that one of them stood on.
    pub fn admit(&mut self, w: &mut World, portal: usize, len_m: f64, dwell_s: u64) -> Option<TrainId> {
        if w.tick % (RETRY_S * TICK_HZ as u64) != 0 {
            return None;
        }
        let mut c = w.clone();
        let id = c.spawn(portal, len_m).ok()?;
        let home = c.trains[&id].next_signal?;
        let ok = self.choices.get(&home)?.iter().any(|&r| {
            let route = &c.lock.routes()[r];
            c.apply(Action::SetRoute { entry: route.entry, exit: route.exit }).is_ok()
        });
        if !ok {
            return None;
        }
        *w = c;
        self.dwell_s.insert(id, dwell_s);
        self.decided(w.tick);
        Some(id)
    }

    /// Call once per tick, after the tick's movement.
    pub fn step(&mut self, w: &mut World) {
        let now = w.tick;
        let occ = w.occupancy();
        // Start each dwell the tick a train comes to rest at its Starter.
        for (&id, t) in &w.trains {
            let Some(s) = t.next_signal else { continue };
            if w.yard.signals[s].kind == SignalKind::Starter && t.v == 0.0 && t.to_signal(&w.yard, s).is_some_and(|d| d < 1.0) && !self.dwell_until.contains_key(&id) {
                let d = self.dwell_s.get(&id).copied().unwrap_or(0);
                self.dwell_until.insert(id, now + d * TICK_HZ as u64);
            }
        }
        self.dwell_until.retain(|id, _| w.trains.contains_key(id));
        self.waiting_since.retain(|(id, _), _| w.trains.contains_key(id));
        self.dwell_s.retain(|id, _| w.trains.contains_key(id));

        if now % (RETRY_S * TICK_HZ as u64) != 0 {
            return;
        }
        let waiting: Vec<(TrainId, SignalId)> = w
            .trains
            .iter()
            .filter_map(|(&id, t)| {
                let s = t.next_signal?;
                if t.route.is_some() || w.lock.takeable(s).is_some() {
                    return None;
                }
                let ready = match w.yard.signals[s].kind {
                    SignalKind::Home => true,
                    SignalKind::Starter => self.dwell_until.get(&id).is_some_and(|&u| now >= u),
                };
                ready.then_some((id, s))
            })
            .collect();
        for (id, s) in waiting {
            let since = *self.waiting_since.entry((id, s)).or_insert(now);
            for &r in self.choices.get(&s).map(Vec::as_slice).unwrap_or(&[]) {
                let route = w.lock.routes()[r].clone();
                match w.apply(Action::SetRoute { entry: route.entry, exit: route.exit }) {
                    Ok(()) => {
                        self.decided(now);
                        let wait = (now - since) / TICK_HZ as u64;
                        if w.yard.signals[s].kind == SignalKind::Home {
                            self.max_wait_home_s = self.max_wait_home_s.max(wait);
                        } else if wait > self.max_wait_starter_s {
                            self.max_wait_starter_s = wait;
                            self.max_wait_starter = Some(s);
                        }
                        break;
                    }
                    Err(e) => {
                        *self.refusals.entry(kind(&e)).or_insert(0) += 1;
                        if let Refusal::Occupied { at: Res::Point(_) | Res::Crossing(_), by } = e
                            && w.trains[&by].next_signal.is_some_and(|x| w.yard.signals[x].kind == SignalKind::Starter)
                            && w.trains[&by].v == 0.0
                        {
                            self.fouled_by_berthed += 1;
                        }
                        if !self.path_free(w, &occ, &route.path, route.entry) {
                            self.path_blocked += 1;
                            if self.free_alt(w, &occ, &route) {
                                self.alt_free += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Whether a junction or edge on a path is free: no live lock on it, no
    /// body on it, no body within FOULING_M of a junction on another edge.
    /// The tests `Interlocking::request` makes, for the path alone.
    fn edge_free(&self, w: &World, occ: &Occupancy, e: usize, entry: Option<SignalId>) -> bool {
        let len = w.yard.edges[e].len;
        let (lo, hi) = match entry.map(|s| &w.yard.signals[s]) {
            Some(s) if s.forward => (s.offset, len),
            Some(s) => (0.0, s.offset),
            None => (0.0, len),
        };
        !w.lock.held(Res::Edge(e)) && occ.on(e, lo, hi).is_none()
    }

    fn node_free(&self, w: &World, occ: &Occupancy, n: usize, from: usize, to: usize) -> bool {
        match junction(&self.g, n) {
            Some(res) => !w.lock.held(res) && occ.near(&w.yard, n, FOULING_M, &[from, to]).is_none(),
            None => true,
        }
    }

    fn path_free(&self, w: &World, occ: &Occupancy, path: &[usize], entry: SignalId) -> bool {
        let y = &w.yard;
        path.iter().enumerate().all(|(i, &e)| self.edge_free(w, occ, e, (i == 0).then_some(entry)))
            && path.windows(2).all(|p| {
                let (x, z) = (&y.edges[p[0]], &y.edges[p[1]]);
                let n = if x.a == z.a || x.a == z.b { x.a } else { x.b };
                self.node_free(w, occ, n, p[0], p[1])
            })
    }

    /// A path from the route's entry to its exit over free running track,
    /// passing no other signal for its direction: what a "via" click could
    /// set. Path only; the overlap and the block are the same for any path
    /// to the same exit.
    fn free_alt(&self, w: &World, occ: &Occupancy, r: &Route) -> bool {
        let y: &Yard = &w.yard;
        let s = &y.signals[r.entry];
        let along = |e: usize, f: bool, off: f64| if f { off } else { y.edges[e].len - off };
        let start = (s.edge, s.forward);
        if !self.edge_free(w, occ, s.edge, Some(r.entry)) {
            return false;
        }
        let mut seen = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some((e, f)) = queue.pop_front() {
            let from = if (e, f) == start { along(e, f, s.offset) + EPS } else { f64::NEG_INFINITY };
            let ahead = self.signals_on.get(&(e, f)).and_then(|l| l.iter().copied().filter(|&k| along(e, f, y.signals[k].offset) > from).min_by(|&a, &b| along(e, f, y.signals[a].offset).total_cmp(&along(e, f, y.signals[b].offset))));
            if let Some(x) = ahead {
                if matches!(r.exit, Exit::Berth { starter, .. } if starter == x) {
                    return true;
                }
                continue;
            }
            let n = y.head(e, f);
            if let Some(p) = y.portals.iter().position(|p| p.edge == e && p.node == n) {
                if r.exit == Exit::Portal(p) {
                    return true;
                }
                continue;
            }
            for nx in self.g.next(y, e, f) {
                if y.edges[nx.0].kind != Kind::Running || seen.contains(&nx) {
                    continue;
                }
                if self.node_free(w, occ, n, e, nx.0) && self.edge_free(w, occ, nx.0, None) {
                    seen.insert(nx);
                    queue.push_back(nx);
                }
            }
        }
        false
    }

    /// For a failure message: each train, where it stands, and why each
    /// route it could take is refused, tried on a copy of the world.
    pub fn explain(&self, w: &World) -> String {
        let y = &w.yard;
        let mut out = String::new();
        for (&id, t) in &w.trains {
            let at = t.next_signal.map_or("leaving".to_string(), |s| y.signals[s].name.clone());
            out += &format!("\n  train {id} ({:.0} m) at {at}, v {:.1}, route {:?}, heads on edge {}", t.len_m, t.v, t.route, t.run[t.head].0);
            for &r in t.next_signal.and_then(|s| self.choices.get(&s)).map(Vec::as_slice).unwrap_or(&[]) {
                let route = &w.lock.routes()[r];
                let exit = match route.exit {
                    Exit::Berth { starter, .. } => y.signals[starter].name.clone(),
                    Exit::Portal(p) => y.portals[p].name.clone(),
                };
                let got = w.clone().apply(Action::SetRoute { entry: route.entry, exit: route.exit });
                out += &format!("\n    to {exit}: {got:?}");
            }
        }
        out
    }

    /// Seconds since the last route was set, when any train still waits
    /// for one.
    pub fn stalled_s(&self, w: &World) -> u64 {
        if w.trains.is_empty() { 0 } else { (w.tick - self.last_decision) / TICK_HZ as u64 }
    }
}
