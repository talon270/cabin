//! B10 (PLAN-redesign.md B3): the ladder levels. Each level's yard parses,
//! each scripted train is due at a portal whose Home can route it into a
//! platform, and a plain controller that does only what a player does
//! (Accept, then set a route from the signal a train waits at) finishes
//! every level inside the shift, with the invariant checker on every tick.

mod common;
use cabin_sim::consts::{SHIFT_LEN_S, TICK_HZ};
use cabin_sim::level::LEVELS;
use cabin_sim::route::Exit;
use cabin_sim::yard::SignalKind;
use cabin_sim::{Action, World};
use common::invariant::Checker;

/// Try each route from `s`, berths first; keep the first the interlocking
/// grants.
fn set_any(w: &mut World, s: usize) -> bool {
    let mut routes: Vec<_> = w.lock.routes().iter().filter(|r| r.entry == s).cloned().collect();
    routes.sort_by_key(|r| !matches!(r.exit, Exit::Berth { .. }));
    routes.into_iter().any(|r| w.apply(Action::SetRoute { entry: r.entry, exit: r.exit }).is_ok())
}

/// One controller decision round, as a player at the panel: accept an
/// offer only when its train could be routed in at once (tried on a copy),
/// then route every train waiting at a Home, or at a Starter after its dwell.
fn play(w: &mut World) {
    let portals: Vec<usize> = w.offers.iter().filter(|(_, q)| !q.is_empty()).map(|(&p, _)| p).collect();
    for p in portals {
        let mut c = w.clone();
        if c.apply(Action::Accept(p)).is_err() {
            continue;
        }
        let Some(id) = c.trains.keys().max().copied() else { continue };
        let Some(home) = c.trains[&id].next_signal else { continue };
        if set_any(&mut c, home) {
            *w = c;
        }
    }
    let waiting: Vec<usize> = w.trains.values().filter_map(|t| {
        let s = t.next_signal?;
        if t.route.is_some() || w.lock.takeable(s).is_some() {
            return None;
        }
        let ready = match w.yard.signals[s].kind {
            SignalKind::Home => true,
            SignalKind::Starter => t.dwell_until.is_some_and(|u| w.tick >= u),
        };
        ready.then_some(s)
    }).collect();
    for s in waiting {
        set_any(w, s);
    }
}

#[test]
fn every_level_parses_and_its_portals_exist() {
    for l in &LEVELS {
        let w = World::level(l).unwrap_or_else(|e| panic!("{e}"));
        assert!(!l.trains.is_empty() && l.trains.iter().all(|t| t.number.starts_with('L')), "{}: invented trains are numbered L…", l.code);
        println!("{}: {} platforms, {} portals, {} signals, {} routes, {} trains",
            l.station, w.yard.platforms.len(), w.yard.portals.len(), w.yard.signals.len(), w.lock.routes().len(), l.trains.len());
    }
}

#[test]
fn every_level_can_be_finished_by_routing_alone() {
    for l in &LEVELS {
        let mut w = World::level(l).unwrap();
        let mut ck = Checker::new(&w);
        let mut done_at = None;
        for tick in 0..SHIFT_LEN_S * TICK_HZ {
            if tick % TICK_HZ == 0 {
                play(&mut w);
            }
            w.step_ticks(1);
            ck.check(&w);
            if w.level_done() {
                done_at = Some(tick / TICK_HZ);
                break;
            }
        }
        let s = done_at.unwrap_or_else(|| panic!("{}: not finished in the shift; handled {} of {}", l.code, w.score.handled, l.trains.len()));
        println!("{}: all {} trains handled in {}:{:02} game time, put-backs {}, delay {:.1} min per train",
            l.code, l.trains.len(), s / 60, s % 60, w.score.put_backs, w.score.delay_per_train());
        assert_eq!(w.score.put_backs, 0, "{}: the controller never cancels", l.code);
    }
}

#[test]
fn a_level_is_not_done_before_its_last_train_leaves() {
    let mut w = World::level(&LEVELS[0]).unwrap();
    assert!(!w.level_done());
    w.step_ticks(60 * TICK_HZ);
    assert!(!w.level_done(), "nothing handled yet");
}
