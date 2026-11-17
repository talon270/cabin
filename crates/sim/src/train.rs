//! CABIN · TRAINS
//!   · Train: a body of fixed length on a run of directed edges, its speed
//!     and the end of its movement authority
//!   · Body, Occupancy: every train's body as spans of edges, built each
//!     tick; the interlocking reads these and never reads `Train`
//!
//! A train's run is every edge it has been given, in order: the approach
//! from its portal, then the path of each route it took. Its authority ends
//! at the next signal unless it has taken that signal's route, and it
//! brakes to stop there: a train never passes a red signal it can stop at.

use std::collections::BTreeMap;

use crate::consts::{ACCEL_MS2, BRAKE_MS2, SPEED_NORMAL_KMH, TICK_HZ};
use crate::route::{PortalId, RouteId, SignalId};
use crate::yard::Yard;

pub type TrainId = usize;

/// Positions are stored with one decimal, so two numbers a millimetre
/// apart are the same place.
pub const EPS: f64 = 1e-3;

/// Part of one edge a body covers, in metres from the edge's `a` end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub edge: usize,
    pub lo: f64,
    pub hi: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    /// Tail to head.
    pub spans: Vec<Span>,
    /// The head: its edge and metres from that edge's `a` end.
    pub head: (usize, f64),
    pub stopped: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Occupancy {
    pub bodies: BTreeMap<TrainId, Body>,
}

impl Occupancy {
    /// A train whose body covers any of `edge` between `lo` and `hi`.
    pub fn on(&self, edge: usize, lo: f64, hi: f64) -> Option<TrainId> {
        self.bodies.iter().find(|(_, b)| b.spans.iter().any(|s| s.edge == edge && s.lo < hi - EPS && s.hi > lo + EPS)).map(|(&t, _)| t)
    }

    /// A train within `m` metres of `node` on an incident edge not in
    /// `except`.
    pub fn near(&self, yard: &Yard, node: usize, m: f64, except: &[usize]) -> Option<TrainId> {
        self.bodies.iter().find(|(_, b)| crate::interlock::body_near(yard, b, node, m, except)).map(|(&t, _)| t)
    }
}

/// The fastest speed that still stops within `d` metres braking tick by
/// tick at BRAKE_MS2: v²/2b + v·dt/2 = d. The continuous √(2bd) brakes 13%
/// too hard at 2 Hz.
pub fn stop_speed(d: f64) -> f64 {
    let h = BRAKE_MS2 / TICK_HZ as f64 / 2.0;
    -h + (h * h + 2.0 * BRAKE_MS2 * d).sqrt()
}

pub fn kmh(v: f64) -> f64 {
    v / 3.6
}

#[derive(Clone, Debug, PartialEq)]
pub struct Train {
    pub id: TrainId,
    pub len_m: f64,
    /// Metres a second.
    pub v: f64,
    pub vmax: f64,
    /// Directed edges, in running order.
    pub run: Vec<(usize, bool)>,
    /// Index into `run` of the head's edge.
    pub head: usize,
    /// Metres along `run[head]` in the direction of travel.
    pub head_m: f64,
    /// Metres of train already past the portal it is leaving by.
    pub gone_m: f64,
    /// Where authority ends: an index into `run` and metres along that
    /// edge. None while leaving: the line beyond the portal is clear.
    pub limit: Option<(usize, f64)>,
    pub next_signal: Option<SignalId>,
    pub leaving: Option<PortalId>,
    /// The route taken at the next signal, until the train is past it.
    pub route: Option<RouteId>,
}

/// Metres along `edge` in direction `forward` of a point `off` metres from
/// its `a` end, and back: the map is its own inverse.
pub fn along(yard: &Yard, edge: usize, forward: bool, off: f64) -> f64 {
    if forward { off } else { yard.edges[edge].len - off }
}

impl Train {
    pub fn new(id: TrainId, len_m: f64, run: Vec<(usize, bool)>) -> Train {
        Train {
            id,
            len_m,
            v: 0.0,
            vmax: kmh(SPEED_NORMAL_KMH),
            run,
            head: 0,
            head_m: 0.0,
            gone_m: 0.0,
            limit: None,
            next_signal: None,
            leaving: None,
            route: None,
        }
    }

    pub fn body(&self, yard: &Yard) -> Body {
        let mut spans = Vec::new();
        let mut left = self.len_m - self.gone_m;
        let (mut i, mut pos) = (self.head as isize, self.head_m);
        while left > EPS && i >= 0 {
            let (e, f) = self.run[i as usize];
            let take = pos.min(left);
            let (lo, hi) = (along(yard, e, f, pos - take), along(yard, e, f, pos));
            let (lo, hi) = if f { (lo, hi) } else { (hi, lo) };
            if hi - lo > EPS || spans.is_empty() {
                spans.push(Span { edge: e, lo, hi });
            }
            left -= take;
            i -= 1;
            if i >= 0 {
                pos = yard.edges[self.run[i as usize].0].len;
            }
        }
        spans.reverse();
        let (he, hf) = self.run[self.head];
        Body { spans, head: (he, along(yard, he, hf, self.head_m)), stopped: self.v == 0.0 }
    }

    /// Metres from the head to the point `at` metres along `run[k]`.
    fn to(&self, yard: &Yard, k: usize, at: f64) -> f64 {
        let mut d = -self.head_m;
        for i in self.head..k {
            d += yard.edges[self.run[i].0].len;
        }
        d + at
    }

    /// Metres from the head to the end of authority; infinite while leaving.
    pub fn to_limit(&self, yard: &Yard) -> f64 {
        self.limit.map_or(f64::INFINITY, |(k, at)| self.to(yard, k, at).max(0.0))
    }

    /// Metres from the head to `signal`, if it lies on the run ahead.
    pub fn to_signal(&self, yard: &Yard, signal: SignalId) -> Option<f64> {
        let s = &yard.signals[signal];
        let k = (self.head..self.run.len()).find(|&i| self.run[i] == (s.edge, s.forward))?;
        Some(self.to(yard, k, along(yard, s.edge, s.forward, s.offset)))
    }

    /// One tick of motion: accelerate towards `vmax`, never faster than
    /// can stop at the end of authority, and never past it.
    pub fn advance(&mut self, yard: &Yard) {
        let dt = 1.0 / TICK_HZ as f64;
        let d = self.to_limit(yard);
        // Over a lowered limit the train brakes down to it; it never jumps.
        let wanted = if self.v > self.vmax { (self.v - BRAKE_MS2 * dt).max(self.vmax) } else { (self.v + ACCEL_MS2 * dt).min(self.vmax) };
        self.v = wanted.min(stop_speed(d));
        let mut step = (self.v * dt).min(d);
        if d - step < EPS {
            self.v = 0.0;
            step = d;
        }
        self.head_m += step;
        loop {
            let len = yard.edges[self.run[self.head].0].len;
            if self.head_m <= len {
                break;
            }
            if self.head + 1 < self.run.len() {
                self.head_m -= len;
                self.head += 1;
            } else {
                // Off the end of the run: only a train leaving gets here.
                self.gone_m += self.head_m - len;
                self.head_m = len;
            }
        }
    }
}
