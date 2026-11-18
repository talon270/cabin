//! B6: what the controls need from the sim. Accepting an offered train, the
//! dwell at a platform, and one line of text for every refusal. The seeded
//! traffic that B6 stood in is tested in `b7_traffic.rs`.

mod common;

use cabin_sim::interlock::{Aspect, Refusal};
use cabin_sim::route::{Exit, Res, Route};
use cabin_sim::yard::SignalKind;
use cabin_sim::{Action, Offer, World};
use common::invariant::Checker;
use common::*;

/// An arrival whose Starter also has a departure.
fn arrival_and_departure(w: &World) -> (Route, Route) {
    let rs = w.lock.routes();
    for r in rs.iter().filter(|r| w.yard.signals[r.entry].kind == SignalKind::Home) {
        let Exit::Berth { starter, .. } = r.exit else { continue };
        if let Some(d) = rs.iter().find(|d| d.entry == starter && matches!(d.exit, Exit::Portal(_))) {
            return (r.clone(), d.clone());
        }
    }
    panic!("no arrival with a departure");
}

fn offer(w: &mut World, p: usize, dwell_s: u32) {
    w.offers.entry(p).or_default().push_back(Offer { len_m: 187.0, dwell_s, since: w.tick, train: None });
}

#[test]
fn refusal_not_offered() {
    let mut w = world();
    assert_eq!(refused(&mut w, Action::Accept(0)), Refusal::NotOffered { portal: 0 });
}

#[test]
fn accept_takes_the_first_offer_and_a_refused_accept_keeps_it() {
    let mut w = world();
    let (r, _) = arrival_and_departure(&w);
    let p = portal_of_home(&w, r.entry);
    offer(&mut w, p, 60);
    offer(&mut w, p, 90);
    w.apply(Action::Accept(p)).expect("first accept");
    assert_eq!(w.offers[&p].len(), 1, "one offer left");
    let t = *w.trains.keys().next().unwrap();
    assert_eq!(w.trains[&t].dwell_s, 60, "the train took the first offer's dwell");
    // The first train still stands on the portal track: the second is
    // refused and stays offered.
    let got = refused(&mut w, Action::Accept(p));
    assert!(matches!(got, Refusal::Occupied { .. }), "{got:?}");
    assert_eq!(w.offers[&p].len(), 1);
}

#[test]
fn a_train_dwells_before_it_takes_a_departure_set_early() {
    let mut w = world();
    let mut ck = Checker::new(&w);
    let (r, d) = arrival_and_departure(&w);
    let p = portal_of_home(&w, r.entry);
    offer(&mut w, p, 120);
    w.apply(Action::Accept(p)).unwrap();
    let t = *w.trains.keys().next().unwrap();
    set(&mut w, &r);
    let Exit::Berth { starter, .. } = r.exit else { unreachable!() };
    run_until(&mut w, &mut ck, 900, |w| stopped_at(w, t, starter));
    w.step_ticks(1);
    let until = w.trains[&t].dwell_until.expect("dwell started at rest");
    set(&mut w, &d);
    assert_eq!(w.aspect(starter), Aspect::Green, "the departure is set");
    // Still standing one tick before the dwell ends.
    while w.tick + 1 < until {
        w.step_ticks(1);
        ck.check(&w);
    }
    assert!(stopped_at(&w, t, starter), "left before its dwell ended");
    run_until(&mut w, &mut ck, 900, |w| !w.trains.contains_key(&t));
}

#[test]
fn every_refusal_reads_as_one_line_naming_what_is_in_the_way() {
    let w = world();
    let (r, _) = arrival_and_departure(&w);
    let a = Action::SetRoute { entry: r.entry, exit: r.exit };
    let pl = &w.yard.platforms[0];
    let cases = [
        (Refusal::NoSuchRoute, "no such route"),
        (Refusal::Conflict { with: r.id, at: Res::Point(3) }, "holds points 3"),
        (Refusal::Occupied { at: Res::Edge(pl.edge), by: 0 }, &format!("T1 stands on or foul of the platform {} line", pl.name) as &str),
        (Refusal::OverlapOccupied { at: Res::Crossing(2), by: 4 }, "T5 stands in the overlap, on or foul of crossing 2"),
        (Refusal::NoLineClear { portal: 0, wait_s: 95 }, &format!("no Line Clear beyond {} for 1:35", w.yard.portals[0].name)),
        (Refusal::TrainPastSignal { train: 1 }, "T2 is already past the signal"),
        (Refusal::ApproachLocked { release_in_s: 120 }, "approach locked, released in 2:00"),
        (Refusal::NotOffered { portal: 1 }, &format!("no train is offered at {}", w.yard.portals[1].name)),
    ];
    for (refusal, want) in cases {
        let got = w.describe(a, refusal);
        assert!(got.starts_with(&format!("{}: refused, ", w.route_name(r.id))), "{got}");
        assert!(got.contains(want), "{got} lacks {want}");
        assert!(!got.contains('\n'));
    }
    assert!(w.describe(Action::CancelRoute(r.id), Refusal::NoSuchRoute).contains("that route is not set"));
}
