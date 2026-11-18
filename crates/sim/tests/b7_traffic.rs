//! B7: the schedule, the modes and the score. Same seed, same schedule;
//! rates within 5% of the profile over 10 hours; the draws follow the
//! profile's shares; and each of the three numbers counts what it says and
//! nothing else.

mod common;

use std::collections::BTreeSet;

use cabin_sim::consts::{PRACTICE_MAX, PRACTICE_MIN, PRACTICE_SPREAD_S, SHIFT_LEN_S, SHIFT_START_S, TICK_HZ};
use cabin_sim::route::{Exit, Route};
use cabin_sim::traffic::{Mode, NDLS_TRAFFIC, Profile, schedule};
use cabin_sim::yard::{SignalKind, Yard};
use cabin_sim::{Action, Offer, World};
use common::invariant::Checker;
use common::*;

fn profile() -> Profile {
    Profile::parse(NDLS_TRAFFIC).expect("data/ndls.traffic parses")
}

fn shift(mode: Mode, seed: u64) -> World {
    World::shift(Yard::parse(cabin_sim::NDLS).unwrap(), profile(), mode, seed)
}

const SCORED: [Mode; 3] = [Mode::Light, Mode::Normal, Mode::Peak];

#[test]
fn the_profile_parses_with_its_pool() {
    let p = profile();
    assert_eq!(p.hourly.iter().sum::<u32>(), 275);
    assert_eq!(p.pool.len(), 139);
    assert!(p.pool.iter().all(|t| t.len_m > 150.0 && t.len_m < 600.0), "a rake length out of range");
}

#[test]
fn same_seed_same_schedule_and_another_seed_differs() {
    let p = profile();
    for m in Mode::ALL {
        let a = schedule(&p, m, 7, SHIFT_START_S, SHIFT_LEN_S);
        assert_eq!(a, schedule(&p, m, 7, SHIFT_START_S, SHIFT_LEN_S), "{m:?}");
        assert_ne!(a, schedule(&p, m, 8, SHIFT_START_S, SHIFT_LEN_S), "{m:?}: seeds 7 and 8");
    }
    // The whole world too, an hour in, with nobody at the panel.
    let (mut a, mut b) = (shift(Mode::Peak, 7), shift(Mode::Peak, 7));
    a.step_ticks(3600 * TICK_HZ);
    b.step_ticks(3600 * TICK_HZ);
    assert!(a == b, "two runs of seed 7 differ");
}

#[test]
fn rates_are_within_5_percent_of_the_profile_over_10_hours() {
    let p = profile();
    let (start, hours) = (6 * 3600, 10);
    for m in SCORED {
        let want: f64 = (6..6 + hours).map(|h| f64::from(p.hourly[h]) * m.fraction()).sum();
        for seed in 1..=3 {
            let s = schedule(&p, m, seed, start, hours as u32 * 3600);
            let got = s.len() as f64;
            println!("{:?} seed {seed}: {got} trains in 10 h, profile {want}", m);
            assert!((got - want).abs() <= 0.05 * want, "{m:?} seed {seed}: {got} against {want}");
            // And hour by hour, within one train of the profile.
            for h in 6..6 + hours as u32 {
                let n = s.iter().filter(|d| d.at_s / 3600 == h).count() as f64;
                let w = f64::from(p.hourly[h as usize]) * m.fraction();
                // The first train is moved to the start, so hour 6 may gain one.
                assert!((n - w).abs() <= 1.0 + f64::from(u8::from(h == 6)), "{m:?} hour {h}: {n} against {w}");
            }
        }
    }
}

#[test]
fn draws_follow_the_profile_shares() {
    let p = profile();
    let (mut n, mut north) = (0.0, 0.0);
    let mut class = vec![0.0; p.classes.len()];
    for seed in 1..=20 {
        for d in schedule(&p, Mode::Peak, seed, 0, 24 * 3600) {
            n += 1.0;
            north += f64::from(u8::from(d.north));
            let c = p.pool[d.train].class;
            class[c] += 1.0;
            let (lo, hi) = (p.classes[c].dwell.0.min(p.terminating.0), p.classes[c].dwell.1.max(p.terminating.1));
            assert!((lo..=hi).contains(&d.dwell_s), "dwell {} outside {lo}..{hi}", d.dwell_s);
        }
    }
    println!("{n} trains: north {:.3}, class {:?}", north / n, class.iter().map(|c| (c / n * 1000.0).round() / 1000.0).collect::<Vec<_>>());
    assert!((north / n - p.north_share).abs() < 0.03, "north {}", north / n);
    for (k, c) in p.classes.iter().enumerate() {
        assert!((class[k] / n - c.share).abs() < 0.03, "{}: {} against {}", c.name, class[k] / n, c.share);
    }
}

#[test]
fn no_train_number_repeats_within_a_shift() {
    let p = profile();
    for m in SCORED {
        for seed in 1..=20 {
            let s = schedule(&p, m, seed, SHIFT_START_S, SHIFT_LEN_S);
            let numbers: BTreeSet<&str> = s.iter().map(|d| p.pool[d.train].number.as_str()).collect();
            assert_eq!(numbers.len(), s.len(), "{m:?} seed {seed}");
        }
    }
}

#[test]
fn a_shift_opens_with_a_train_offered_by_its_number() {
    for m in Mode::ALL {
        let w = shift(m, 3);
        let offers: Vec<&Offer> = w.offers.values().flatten().collect();
        assert_eq!(offers.len(), 1, "{m:?}: one train offered at the start");
        let (number, name) = w.offer_name(offers[0]);
        assert!(!number.is_empty() && !name.is_empty(), "{m:?}");
    }
    let mut w = shift(Mode::Normal, 3);
    let (&p, q) = w.offers.iter().find(|(_, q)| !q.is_empty()).unwrap();
    let (number, _) = w.offer_name(&q[0]);
    w.apply(Action::Accept(p)).unwrap();
    let t = *w.trains.keys().next().unwrap();
    assert_eq!(w.train_name(t), number);
}

#[test]
fn practice_offers_2_to_6_trains_and_keeps_no_score() {
    let p = profile();
    for seed in 1..=30 {
        let s = schedule(&p, Mode::Practice, seed, SHIFT_START_S, SHIFT_LEN_S);
        assert!((PRACTICE_MIN as usize..=PRACTICE_MAX as usize).contains(&s.len()), "seed {seed}: {}", s.len());
        assert!(s.iter().all(|d| d.at_s < SHIFT_START_S + PRACTICE_SPREAD_S));
    }
    let mut w = shift(Mode::Practice, 1);
    w.step_ticks((SHIFT_LEN_S + 600) * TICK_HZ);
    assert_eq!(w.score, Default::default(), "Practice scored");
}

#[test]
fn a_scored_shift_ends_at_its_length_and_the_score_stops() {
    let mut w = shift(Mode::Light, 2);
    w.step_ticks(SHIFT_LEN_S * TICK_HZ - 1);
    assert!(w.score.over.is_none(), "over early");
    w.step_ticks(1);
    assert_eq!(w.score.over, Some(u64::from(SHIFT_LEN_S * TICK_HZ)));
    assert!(w.score.held_ticks > 0, "trains left outside all shift held nothing");
    let held = w.score.clone();
    w.step_ticks(600 * TICK_HZ);
    assert_eq!(w.score, held, "the score moved after the shift");
    assert_eq!(w.clock_s(), u64::from(SHIFT_START_S + SHIFT_LEN_S + 600));
}

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

/// The test world, scored as a Normal shift with no schedule.
fn scored_world() -> World {
    let mut w = world();
    w.mode = Some(Mode::Normal);
    w
}

#[test]
fn delay_counts_waits_outside_and_at_red_and_never_the_dwell() {
    let mut w = scored_world();
    let mut ck = Checker::new(&w);
    let (r, d) = arrival_and_departure(&w);
    let Exit::Berth { starter, .. } = r.exit else { unreachable!() };
    let p = portal_of_home(&w, r.entry);
    w.offers.entry(p).or_default().push_back(Offer { len_m: 264.0, dwell_s: 120, since: 0, train: None });
    w.step_ticks(10);
    assert_eq!(w.score.held_ticks, 10, "offered and not accepted: one tick a tick");
    w.apply(Action::Accept(p)).unwrap();
    let t = *w.trains.keys().next().unwrap();
    // Running in to the Home is not delay; standing at it is.
    run_until(&mut w, &mut ck, 900, |w| stopped_at(w, t, r.entry));
    let h = w.score.held_ticks;
    w.step_ticks(20);
    assert_eq!(w.score.held_ticks, h + 20, "standing at a red Home");
    set(&mut w, &r);
    run_until(&mut w, &mut ck, 900, |w| stopped_at(w, t, starter));
    w.step_ticks(1);
    let until = w.trains[&t].dwell_until.expect("dwell started");
    let h = w.score.held_ticks;
    while w.tick + 1 < until {
        w.step_ticks(1);
    }
    assert_eq!(w.score.held_ticks, h, "the dwell counted as delay");
    w.step_ticks(21);
    assert_eq!(w.score.held_ticks, h + 21, "standing at a red Starter after the dwell");
    assert_eq!(w.score.handled, 0);
    set(&mut w, &d);
    run_until(&mut w, &mut ck, 900, |w| !w.trains.contains_key(&t));
    assert_eq!(w.score.handled, 1, "the train left by a portal");
}

#[test]
fn a_put_back_is_a_cancel_in_the_face_of_a_train_only() {
    let mut w = scored_world();
    let mut ck = Checker::new(&w);
    let (r, d) = arrival_and_departure(&w);
    // No train anywhere near: a cancel, not a put-back.
    set(&mut w, &d);
    w.apply(Action::CancelRoute(d.id)).unwrap();
    assert_eq!(w.score.put_backs, 0, "a cancel with no train counted");
    // A train standing at the Home, its route set and not yet taken.
    let t = w.spawn(portal_of_home(&w, r.entry), 264.0).unwrap();
    run_until(&mut w, &mut ck, 900, |w| stopped_at(w, t, r.entry));
    set(&mut w, &r);
    w.apply(Action::CancelRoute(r.id)).unwrap();
    assert_eq!(w.score.put_backs, 1, "put back in the face of a train");
    // A refused cancel counts nothing: the route is no longer set.
    let before = w.score.clone();
    let _ = refused(&mut w, Action::CancelRoute(r.id));
    assert_eq!(w.score, before);
}

#[test]
fn delay_is_shown_per_train_due() {
    for m in SCORED {
        let mut w = shift(m, 1);
        let n = schedule(&profile(), m, 1, SHIFT_START_S, SHIFT_LEN_S).len() as u32;
        w.step_ticks(SHIFT_LEN_S * TICK_HZ);
        assert!(w.score.over.is_some());
        assert_eq!(w.score.due, n, "{}: every scheduled train falls due in the shift", m.name());
        let per = w.score.delay_per_train();
        assert!((per * f64::from(n) - w.score.delay_min()).abs() < 1e-9, "{}: {per} per train over {n}", m.name());
        println!("{}: {n} due, {:.1} min, {per:.1} min per train", m.name(), w.score.delay_min());
    }
    assert_eq!(shift(Mode::Practice, 1).score.due, 0, "Practice keeps no score");
}
