//! B4: the guard and train motion.
//!
//! Every refusal is applied to a cloned world and the two compared: a
//! refused request changes nothing. Worst inputs 1 to 5 of the plan's
//! Guard 1 are named `w1_` to `w5_`. The invariant checker runs after every
//! tick of every test that moves a train.

mod common;

use cabin_sim::consts::{APPROACH_RELEASE_S, BLOCK_HEADWAY_S, BRAKE_MS2, TICK_HZ};
use cabin_sim::interlock::{Aspect, Refusal};
use cabin_sim::route::{Exit, Lie, Res, Route};
use cabin_sim::yard::SignalKind;
use cabin_sim::{Action, World};
use common::invariant::Checker;
use common::*;

/// A Rajdhani rake and its loco: 528 m + 20.6 m (`data/ndls.traffic.toml`).
const LONG_M: f64 = 548.6;
/// A MEMU-sized rake.
const SHORT_M: f64 = 187.0;

fn arrivals(w: &World) -> Vec<Route> {
    w.lock.routes().iter().filter(|r| w.yard.signals[r.entry].kind == SignalKind::Home && matches!(r.exit, Exit::Berth { .. })).cloned().collect()
}

fn departures_from(w: &World, starter: usize) -> Vec<Route> {
    w.lock.routes().iter().filter(|r| r.entry == starter && matches!(r.exit, Exit::Portal(_))).cloned().collect()
}

/// An arrival whose Starter also has a departure.
fn arrival_with_departure(w: &World) -> (Route, Route) {
    for r in arrivals(w) {
        let Exit::Berth { starter, .. } = r.exit else { continue };
        if let Some(d) = departures_from(w, starter).into_iter().next() {
            return (r, d);
        }
    }
    panic!("no arrival whose Starter has a departure");
}

// ---- motion -----------------------------------------------------------

#[test]
fn an_arrival_runs_to_its_berth_and_stops_at_the_starter() {
    let mut w = world();
    let mut ck = Checker::new(&w);
    let (r, _) = arrival_with_departure(&w);
    let t = berth(&mut w, &mut ck, &r, SHORT_M);
    let Exit::Berth { starter, .. } = r.exit else { unreachable!() };
    assert!(stopped_at(&w, t, starter));
    // C3-137 "most restrictive aspect": the Home is back at red behind it.
    assert_eq!(w.aspect(r.entry), Aspect::Red);
}

#[test]
fn a_train_with_no_route_stops_at_the_home() {
    let mut w = world();
    let mut ck = Checker::new(&w);
    let home = arrivals(&w)[0].entry;
    let t = w.spawn(portal_of_home(&w, home), SHORT_M).expect("spawn");
    run_until(&mut w, &mut ck, 600, |w| stopped_at(w, t, home));
    w.step_ticks(60 * TICK_HZ);
    ck.check(&w);
    assert!(stopped_at(&w, t, home), "the train crept past a red Home");
}

#[test]
fn sections_release_behind_the_tail_and_points_go_back_to_normal() {
    let mut w = world();
    let mut ck = Checker::new(&w);
    let (r, d) = arrival_with_departure(&w);
    let t = berth(&mut w, &mut ck, &r, SHORT_M);
    // At rest, only what the body still covers stays locked.
    let body = w.occupancy().bodies[&t].clone();
    for &e in r.path.iter().chain(&r.overlap) {
        if !body.spans.iter().any(|s| s.edge == e) {
            assert!(!w.lock.held(Res::Edge(e)), "edge {e} still locked behind a berthed train");
        }
    }
    set(&mut w, &d);
    run_until(&mut w, &mut ck, 900, |w| !w.trains.contains_key(&t));
    for &res in d.holds.iter().chain(&r.holds) {
        assert!(!w.lock.held(res), "{res:?} still locked after the train left");
    }
    // C3-164 "set for the straight": with no route, every point rests normal.
    for k in 0..w.yard.points.len() {
        assert_eq!(w.lie(k), Lie::Normal, "point {k}");
    }
}

// ---- aspects ----------------------------------------------------------

#[test]
fn aspects_are_red_until_a_route_is_set_then_follow_the_signal_ahead() {
    let mut w = world();
    for s in 0..w.yard.signals.len() {
        assert_eq!(w.aspect(s), Aspect::Red, "{}", w.yard.signals[s].name);
    }
    let (r, d) = arrival_with_departure(&w);
    set(&mut w, &r);
    assert_eq!(w.aspect(r.entry), Aspect::Yellow, "Home into a berth whose Starter is red");
    let mut w = world();
    set(&mut w, &d);
    assert_eq!(w.aspect(d.entry), Aspect::Green, "Starter onto a clear line");
}

#[test]
fn a_route_sets_its_points_and_they_rest_normal_when_it_is_cancelled() {
    let mut w = world();
    let r = w.lock.routes().iter().find(|r| r.points.iter().any(|p| p.1 == Lie::Reverse)).cloned().expect("a route with a reverse point");
    set(&mut w, &r);
    for &(k, lie) in &r.points {
        assert_eq!(w.lie(k), lie);
    }
    w.apply(Action::CancelRoute(r.id)).expect("cancel with no train about");
    for &(k, _) in &r.points {
        assert_eq!(w.lie(k), Lie::Normal);
    }
    assert_eq!(w.aspect(r.entry), Aspect::Red);
}

// ---- one test per refusal ----------------------------------------------

#[test]
fn refusal_no_such_route() {
    let mut w = world();
    let home = arrivals(&w)[0].entry;
    let own = portal_of_home(&w, home);
    assert_eq!(refused(&mut w, Action::SetRoute { entry: home, exit: Exit::Portal(own) }), Refusal::NoSuchRoute);
    assert_eq!(refused(&mut w, Action::CancelRoute(0)), Refusal::NoSuchRoute, "cancelling a route that is not set");
}

#[test]
fn refusal_conflict() {
    let mut w = world();
    let rs = w.lock.routes().to_vec();
    let (a, b) = rs.iter().flat_map(|a| rs.iter().map(move |b| (a, b))).find(|(a, b)| a.entry != b.entry && !a.holds.is_disjoint(&b.holds)).expect("two conflicting routes");
    set(&mut w, a);
    match refused(&mut w, Action::SetRoute { entry: b.entry, exit: b.exit }) {
        Refusal::Conflict { with, at } => {
            assert_eq!(with, a.id);
            assert!(a.holds.contains(&at) && b.holds.contains(&at), "{at:?} is not shared");
        }
        x => panic!("{x:?}"),
    }
}

#[test]
fn refusal_occupied() {
    let mut w = world();
    let mut ck = Checker::new(&w);
    let (r, _) = arrival_with_departure(&w);
    let t = berth(&mut w, &mut ck, &r, SHORT_M);
    match refused(&mut w, Action::SetRoute { entry: r.entry, exit: r.exit }) {
        Refusal::Occupied { by, .. } => assert_eq!(by, t),
        x => panic!("{x:?}"),
    }
}

#[test]
fn refusal_overlap_occupied() {
    // No Starter stands on another route's overlap track, so a standing
    // train blocks an overlap by fouling: berthed 10 m short of a trailing
    // point, it fouls a route whose overlap runs over that point by the
    // other leg.
    let w0 = world();
    let y = &w0.yard;
    let g = y.graph();
    let nodes_of = |r: &Route| -> Vec<usize> {
        let run: Vec<usize> = r.path.iter().chain(&r.overlap).copied().collect();
        run.windows(2).map(|w| {
            let (x, z) = (&y.edges[w[0]], &y.edges[w[1]]);
            if x.a == z.a || x.a == z.b { x.a } else { x.b }
        }).collect()
    };
    let rs = arrivals(&w0);
    let (a, b) = rs
        .iter()
        .flat_map(|a| rs.iter().map(move |b| (a, b)))
        .find(|(a, b)| {
            let Exit::Berth { starter, .. } = a.exit else { return false };
            let x = &y.signals[starter];
            let ahead = if x.forward { y.edges[x.edge].b } else { y.edges[x.edge].a };
            let in_overlap = nodes_of(b)[b.path.len() - 1..].contains(&ahead);
            let clear = !b.path.iter().chain(&b.overlap).any(|&e| e == x.edge || a.path.contains(&e));
            matches!(g.at[ahead], cabin_sim::yard::Junction::Point(_)) && in_overlap && clear
        })
        .map(|(a, b)| (a.clone(), b.clone()))
        .expect("an arrival whose overlap runs over the point beside another berth");
    let mut w = w0.clone();
    let mut ck = Checker::new(&w);
    let t = berth(&mut w, &mut ck, &a, SHORT_M);
    match refused(&mut w, Action::SetRoute { entry: b.entry, exit: b.exit }) {
        Refusal::OverlapOccupied { by, at: Res::Point(_) } => assert_eq!(by, t),
        x => panic!("{x:?}"),
    }
}

#[test]
fn refusal_train_past_signal() {
    let mut w = world();
    let mut ck = Checker::new(&w);
    let (r, _) = arrival_with_departure(&w);
    let t = w.spawn(portal_of_home(&w, r.entry), LONG_M).expect("spawn");
    set(&mut w, &r);
    // The head 20 m past the Home, the body still across it.
    run_until(&mut w, &mut ck, 900, |w| head_to(w, t, r.entry).is_some_and(|d| d < -20.0));
    let at = w.clone();
    assert_eq!(refused(&mut w, Action::SetRoute { entry: r.entry, exit: r.exit }), Refusal::TrainPastSignal { train: t });
    // C3-134 "until the whole train has passed": no cancel under the train.
    assert_eq!(refused(&mut w, Action::CancelRoute(r.id)), Refusal::TrainPastSignal { train: t });
    assert_eq!(w, at);
}

#[test]
fn refusal_no_line_clear_until_the_headway_has_run() {
    let mut w = world();
    let mut ck = Checker::new(&w);
    let (r, d) = arrival_with_departure(&w);
    let Exit::Portal(p) = d.exit else { unreachable!() };
    let t = berth(&mut w, &mut ck, &r, SHORT_M);
    set(&mut w, &d);
    run_until(&mut w, &mut ck, 900, |w| !w.trains.contains_key(&t));
    // Any route out by the same portal now waits for the block.
    let again = w.lock.routes().iter().find(|x| x.exit == Exit::Portal(p)).cloned().unwrap();
    match refused(&mut w, Action::SetRoute { entry: again.entry, exit: again.exit }) {
        Refusal::NoLineClear { portal, wait_s } => {
            assert_eq!(portal, p);
            assert!(wait_s <= BLOCK_HEADWAY_S && wait_s + 2 >= BLOCK_HEADWAY_S, "wait {wait_s}");
        }
        x => panic!("{x:?}"),
    }
    w.step_ticks(BLOCK_HEADWAY_S * TICK_HZ);
    set(&mut w, &again);
}

/// Found by the B5 soak: a train offered at a portal stands with its head
/// on the box edge and the rest outside, a body of no length inside. A
/// departure out by that portal was set head-on into it. HN5 makes it last:
/// N5's portal edge is 29.1 m, shorter than HOME_SETBACK_M, so the Home
/// stands on the box edge and the train never comes further in.
#[test]
fn refusal_occupied_a_departure_into_a_train_still_entering_by_its_portal() {
    for (portal, starter) in [("N5", "ST12N"), ("N1", "ST4N")] {
        let mut w = world();
        let p = common::portal(&w, portal);
        let t = w.spawn(p, LONG_M).expect("spawn");
        let d = route(&w, sig(&w, starter), Exit::Portal(p));
        match refused(&mut w, Action::SetRoute { entry: d.entry, exit: d.exit }) {
            Refusal::Occupied { by, .. } => assert_eq!(by, t, "{portal}"),
            x => panic!("{portal}: {x:?}"),
        }
    }
}

/// The same fault from the other side: a second train offered at N5 onto
/// the first, which stands on the box edge at HN5.
#[test]
fn a_second_train_is_not_offered_onto_one_still_entering() {
    let mut w = world();
    let p = common::portal(&w, "N5");
    let t = w.spawn(p, LONG_M).expect("spawn");
    w.step_ticks(10);
    assert_eq!(w.spawn(p, LONG_M), Err(Refusal::Occupied { at: Res::Edge(w.yard.portals[p].edge), by: t }));
}

/// Found by the B5 soak: a train stopped at HN5 stands 29.1 m from point
/// 90, inside FOULING_M. A train was offered at N5 while the train before
/// it still had its tail within FOULING_M of that point.
#[test]
fn no_train_is_offered_up_to_a_home_that_fouls_a_junction_in_use() {
    let mut w = world();
    let mut ck = Checker::new(&w);
    let p = common::portal(&w, "N5");
    let edge = w.yard.portals[p].edge;
    let node = { let e = &w.yard.edges[edge]; if e.a == w.yard.portals[p].node { e.b } else { e.a } };
    let t = w.spawn(p, SHORT_M).expect("spawn");
    let r = arrivals(&w).into_iter().find(|r| r.entry == sig(&w, "HN5")).unwrap();
    set(&mut w, &r);
    run_until(&mut w, &mut ck, 900, |w| {
        let o = w.occupancy();
        o.on(edge, 0.0, w.yard.edges[edge].len).is_none() && o.entering(p).is_none() && o.near(&w.yard, node, cabin_sim::consts::FOULING_M, &[]).is_some()
    });
    match w.spawn(p, SHORT_M) {
        Err(Refusal::Occupied { by, .. }) => assert_eq!(by, t),
        Err(Refusal::Conflict { .. }) => {}
        x => panic!("{x:?}"),
    }
}

#[test]
fn refusal_approach_locked_then_released() {
    // Worst input 3: a route cancelled in the face of a train, then a
    // conflicting route requested at once.
    let mut w = world();
    let mut ck = Checker::new(&w);
    let (r, _) = arrival_with_departure(&w);
    let t = w.spawn(portal_of_home(&w, r.entry), SHORT_M).expect("spawn");
    set(&mut w, &r);
    w.step_ticks(1);
    ck.check(&w);
    w.apply(Action::CancelRoute(r.id)).expect("cancel puts the signal back");
    assert_eq!(w.aspect(r.entry), Aspect::Red);
    let conflicting = w.lock.routes().iter().find(|x| x.entry == r.entry).cloned().unwrap();
    match refused(&mut w, Action::SetRoute { entry: conflicting.entry, exit: conflicting.exit }) {
        Refusal::ApproachLocked { release_in_s } => assert!(release_in_s <= APPROACH_RELEASE_S && release_in_s + 2 >= APPROACH_RELEASE_S),
        x => panic!("{x:?}"),
    }
    assert!(matches!(refused(&mut w, Action::CancelRoute(r.id)), Refusal::ApproachLocked { .. }));
    // The train was far out: it stops at the red Home, and after the timer
    // the route can be set again.
    run_until(&mut w, &mut ck, 900, |w| stopped_at(w, t, r.entry));
    run_until(&mut w, &mut ck, APPROACH_RELEASE_S + 1, |w| !w.lock.held(Res::Edge(r.path[1.min(r.path.len() - 1)])));
    set(&mut w, &r);
}

#[test]
fn a_route_put_back_too_late_to_stop_stays_locked_and_the_train_runs_on() {
    let mut w = world();
    let mut ck = Checker::new(&w);
    let (r, _) = arrival_with_departure(&w);
    let Exit::Berth { starter, .. } = r.exit else { unreachable!() };
    let t = w.spawn(portal_of_home(&w, r.entry), SHORT_M).expect("spawn");
    set(&mut w, &r);
    // Inside its braking distance of the Home, still short of it.
    run_until(&mut w, &mut ck, 900, |w| {
        let v = w.trains[&t].v;
        head_to(w, t, r.entry).is_some_and(|d| d > 0.0 && d < v * v / (2.0 * BRAKE_MS2) - 5.0)
    });
    w.apply(Action::CancelRoute(r.id)).expect("cancel is accepted");
    assert_eq!(w.aspect(r.entry), Aspect::Red);
    // The locks stand while the train runs through; the checker's rule 3
    // fails the test if it ever moves on an unlocked edge.
    run_until(&mut w, &mut ck, 900, |w| stopped_at(w, t, starter));
}

// ---- worst inputs ------------------------------------------------------

#[test]
fn w1_two_routes_over_one_diamond_in_the_same_tick() {
    let mut w = world();
    let rs = w.lock.routes().to_vec();
    let diamond = |c: usize| w.yard.crossings[c].pairs.len() == 2;
    let (a, b, k) = rs
        .iter()
        .flat_map(|a| rs.iter().map(move |b| (a, b)))
        .filter(|(a, b)| a.entry != b.entry)
        .find_map(|(a, b)| {
            let shared: Vec<Res> = a.holds.intersection(&b.holds).copied().collect();
            match shared[..] {
                [Res::Crossing(k)] if diamond(k) => Some((a.clone(), b.clone(), k)),
                _ => None,
            }
        })
        .expect("two routes that share only a diamond");
    w.apply(Action::SetRoute { entry: a.entry, exit: a.exit }).expect("first route");
    assert_eq!(refused(&mut w, Action::SetRoute { entry: b.entry, exit: b.exit }), Refusal::Conflict { with: a.id, at: Res::Crossing(k) });
}

#[test]
fn w2_two_arrivals_into_one_platform_from_opposite_ends() {
    let w0 = world();
    let rs = arrivals(&w0);
    let starter = |r: &Route| match r.exit {
        Exit::Berth { starter, .. } => starter,
        _ => unreachable!(),
    };
    let platform = |r: &Route| match r.exit {
        Exit::Berth { platform, .. } => platform,
        _ => unreachable!(),
    };
    let (a, b) = rs
        .iter()
        .flat_map(|a| rs.iter().map(move |b| (a, b)))
        .find(|(a, b)| platform(a) == platform(b) && starter(a) != starter(b))
        .map(|(a, b)| (a.clone(), b.clone()))
        .expect("a platform with arrivals at both ends");
    // Same tick: the second is a conflict.
    let mut w = w0.clone();
    set(&mut w, &a);
    assert!(matches!(refused(&mut w, Action::SetRoute { entry: b.entry, exit: b.exit }), Refusal::Conflict { with, .. } if with == a.id));
    // With the first train berthed: the platform is occupied.
    let mut w = w0;
    let mut ck = Checker::new(&w);
    let t = berth(&mut w, &mut ck, &a, SHORT_M);
    assert!(matches!(refused(&mut w, Action::SetRoute { entry: b.entry, exit: b.exit }), Refusal::Occupied { by, .. } | Refusal::OverlapOccupied { by, .. } if by == t));
}

#[test]
fn w4_a_train_longer_than_its_berth_keeps_the_points_behind_it_locked() {
    let w0 = world();
    // An arrival with a point on its path less than the train's length
    // behind the Starter.
    let g = w0.yard.graph();
    let (r, k) = arrivals(&w0)
        .into_iter()
        .find_map(|r| {
            let Exit::Berth { starter, .. } = r.exit else { return None };
            let s = &w0.yard.signals[starter];
            let mut back = if s.forward { s.offset } else { w0.yard.edges[s.edge].len - s.offset };
            for i in (1..r.path.len()).rev() {
                let (x, y) = (&w0.yard.edges[r.path[i - 1]], &w0.yard.edges[r.path[i]]);
                let node = if x.a == y.a || x.a == y.b { x.a } else { x.b };
                if let cabin_sim::yard::Junction::Point(k) = g.at[node] {
                    return (back + 5.0 < LONG_M).then_some((r.clone(), k));
                }
                back += x.len;
            }
            None
        })
        .expect("an arrival with a point within a long train's length of its Starter");
    let lie = r.points.iter().find(|p| p.0 == k).unwrap().1;
    let mut w = w0;
    let mut ck = Checker::new(&w);
    let t = berth(&mut w, &mut ck, &r, LONG_M);
    assert!(w.lock.held(Res::Point(k)), "the point under the tail was released");
    assert_eq!(w.lie(k), lie);
    for q in w.lock.routes().to_vec() {
        if q.holds.contains(&Res::Point(k)) && q.entry != r.entry {
            let got = refused(&mut w, Action::SetRoute { entry: q.entry, exit: q.exit });
            // A train this long also stands across its platform's other Starter.
            assert!(matches!(got, Refusal::Conflict { .. } | Refusal::Occupied { .. } | Refusal::OverlapOccupied { .. } | Refusal::TrainPastSignal { .. }), "{got:?}");
        }
    }
    assert_eq!(w.lie(k), lie);
    let _ = t;
}

#[test]
fn w5_a_route_in_rear_while_the_train_ahead_straddles_three_sections() {
    let w0 = world();
    let r = arrivals(&w0).into_iter().find(|r| r.path.len() >= 4).expect("an arrival over 4 or more edges");
    let mut w = w0;
    let mut ck = Checker::new(&w);
    let t = w.spawn(portal_of_home(&w, r.entry), LONG_M).expect("spawn");
    set(&mut w, &r);
    // Tail past the Home's edge, body over three edges or more.
    let home_edge = w.yard.signals[r.entry].edge;
    run_until(&mut w, &mut ck, 900, |w| {
        let b = &w.occupancy().bodies[&t];
        let edges: std::collections::BTreeSet<usize> = b.spans.iter().map(|s| s.edge).collect();
        edges.len() >= 3 && !edges.contains(&home_edge)
    });
    let under: Vec<usize> = w.occupancy().bodies[&t].spans.iter().map(|s| s.edge).collect();
    // Every route from the same Home into track the train still covers.
    let rear: Vec<Route> = w.lock.routes().iter().filter(|q| q.entry == r.entry && q.path.iter().any(|e| under.contains(e))).cloned().collect();
    assert!(rear.iter().any(|q| q.id == r.id));
    for q in rear {
        let got = refused(&mut w, Action::SetRoute { entry: q.entry, exit: q.exit });
        assert!(
            matches!(got, Refusal::Conflict { with, .. } if with == r.id) || matches!(got, Refusal::Occupied { by, .. } if by == t),
            "route {} in rear: {got:?}",
            q.id
        );
    }
}

#[test]
fn refusal_overlap_occupied_by_a_body_on_overlap_track() {
    // No train in NDLS comes to rest on overlap track, so the scenario tests
    // never reach this check: hand `request` such a body directly.
    use cabin_sim::train::{Body, Occupancy, Span};
    let w = world();
    let r = arrivals(&w).into_iter().find(|r| r.overlap.iter().any(|e| !r.path.contains(e))).expect("an arrival with an overlap");
    let e = *r.overlap.iter().find(|e| !r.path.contains(e)).unwrap();
    let len = w.yard.edges[e].len;
    let body = Body { spans: vec![Span { edge: e, lo: len / 2.0 - 5.0, hi: len / 2.0 + 5.0 }], head: (e, len / 2.0 + 5.0), stopped: true, entering: None };
    let occ = Occupancy { bodies: [(7, body)].into_iter().collect() };
    let mut lock = w.lock.clone();
    assert_eq!(lock.request(&w.yard, &occ, r.entry, r.exit), Err(Refusal::OverlapOccupied { at: Res::Edge(e), by: 7 }));
    assert_eq!(lock, w.lock, "a refused request changed the lock table");
}

#[test]
fn a_train_brakes_no_harder_than_its_service_brake() {
    let mut w = world();
    let mut ck = Checker::new(&w);
    let (r, _) = arrival_with_departure(&w);
    let Exit::Berth { starter, .. } = r.exit else { unreachable!() };
    let t = w.spawn(portal_of_home(&w, r.entry), SHORT_M).expect("spawn");
    set(&mut w, &r);
    let dt = 1.0 / TICK_HZ as f64;
    let mut v = w.trains[&t].v;
    let mut worst: f64 = 0.0;
    run_until(&mut w, &mut ck, 900, |w| {
        let now = w.trains[&t].v;
        // The last tick snaps a crawl to rest; above 1 m/s it is braking.
        if v > 1.0 {
            worst = worst.max((v - now) / dt);
        }
        v = now;
        stopped_at(w, t, starter)
    });
    assert!(worst > 0.0 && worst <= BRAKE_MS2 + 1e-9, "hardest braking {worst:.3} m/s2");
}
