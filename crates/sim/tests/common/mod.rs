//! Test helpers shared by B4 and B5: the invariant checker and scenario
//! builders. Each test file uses a different subset.
#![allow(dead_code)]

pub mod invariant;
pub mod signaller;

use cabin_sim::interlock::Refusal;
use cabin_sim::route::{Exit, PortalId, Route, SignalId};
use cabin_sim::train::TrainId;
use cabin_sim::yard::Yard;
use cabin_sim::{Action, World};
use invariant::Checker;

pub fn world() -> World {
    World::new(Yard::parse(cabin_sim::NDLS).expect("data/ndls.yard parses"))
}

pub fn sig(w: &World, name: &str) -> SignalId {
    w.yard.signals.iter().position(|s| s.name == name).unwrap_or_else(|| panic!("no signal {name}"))
}

pub fn portal(w: &World, name: &str) -> PortalId {
    w.yard.portals.iter().position(|p| p.name == name).unwrap_or_else(|| panic!("no portal {name}"))
}

/// The portal a Home stands for: Homes are named `H` and the portal.
pub fn portal_of_home(w: &World, home: SignalId) -> PortalId {
    portal(w, &w.yard.signals[home].name[1..])
}

pub fn route(w: &World, entry: SignalId, exit: Exit) -> Route {
    w.lock.routes().iter().find(|r| r.entry == entry && r.exit == exit).cloned().expect("route exists")
}

pub fn set(w: &mut World, r: &Route) {
    w.apply(Action::SetRoute { entry: r.entry, exit: r.exit }).unwrap_or_else(|e| panic!("route {} refused: {e:?}", r.id));
}

/// Apply an action that must be refused, and prove the refusal changed
/// nothing: the whole world compares equal before and after.
pub fn refused(w: &mut World, a: Action) -> Refusal {
    let before = w.clone();
    let got = w.apply(a);
    assert_eq!(*w, before, "a refused {a:?} changed the world");
    got.expect_err("the action should have been refused")
}

/// Step one tick at a time, checking every invariant after each, until
/// `done` holds. Panics after `max_s` game seconds.
pub fn run_until(w: &mut World, ck: &mut Checker, max_s: u32, mut done: impl FnMut(&World) -> bool) {
    for _ in 0..max_s * cabin_sim::consts::TICK_HZ {
        if done(w) {
            return;
        }
        w.step_ticks(1);
        ck.check(w);
    }
    assert!(done(w), "not reached in {max_s} s");
}

/// Metres from the head of `t` to signal `s`, along `s`'s direction; None
/// if the head is not on the signal's edge.
pub fn head_to(w: &World, t: TrainId, s: SignalId) -> Option<f64> {
    let b = w.occupancy().bodies.get(&t)?.clone();
    let sg = &w.yard.signals[s];
    (b.head.0 == sg.edge).then_some(if sg.forward { sg.offset - b.head.1 } else { b.head.1 - sg.offset })
}

pub fn stopped_at(w: &World, t: TrainId, s: SignalId) -> bool {
    let stopped = w.occupancy().bodies.get(&t).is_some_and(|b| b.stopped);
    stopped && head_to(w, t, s).is_some_and(|d| (0.0..1.0).contains(&d))
}

/// A train of `len_m` brought in at the Home's portal and run over `r` to
/// rest at its exit Starter.
pub fn berth(w: &mut World, ck: &mut Checker, r: &Route, len_m: f64) -> TrainId {
    let Exit::Berth { starter, .. } = r.exit else { panic!("route {} is not an arrival", r.id) };
    let t = w.spawn(portal_of_home(w, r.entry), len_m).expect("spawn");
    set(w, r);
    run_until(w, ck, 900, |w| stopped_at(w, t, starter));
    t
}
