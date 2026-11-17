//! B5 fuzz: worst input 6 of Guard 1. 1,000,000 random actions over 3
//! seeds: route requests, cancels, trains offered at portals, and runs of
//! ticks, with the invariant checker after every tick. Half the requests
//! are for a route from a signal a train is waiting at, so trains move and
//! meet; the rest are any route at all.
//!
//! The world starts afresh every EPISODE actions: an arrival into P15 or
//! P16 can never leave (no train reverses in this model), and a yard full
//! of stranded trains tests nothing.
//!
//! Every 50th refused action is applied to a copy and compared: a refusal
//! changes nothing (B4 tests each refusal kind; this samples them at
//! volume).
//!
//! Takes about 2 minutes in release (121 s on 2026-11-17); run with
//! `cargo test --release -p cabin-sim --test b5_fuzz -- --nocapture`.

mod common;

use std::collections::BTreeMap;

use cabin_sim::route::RouteId;
use cabin_sim::{Action, Rng};
use common::invariant::Checker;
use common::world;

const ACTIONS: u64 = 1_000_000;
const SEEDS: [u64; 3] = [11, 12, 13];
const EPISODE: u64 = 10_000;
/// MEMU, EMU, Shatabdi and Rajdhani lengths with loco, from
/// `data/ndls.traffic.toml`.
const LENS: [f64; 4] = [187.0, 264.0, 404.6, 548.6];

#[derive(Default, Debug)]
struct Counts {
    set: u64,
    cancelled: u64,
    offered: u64,
    left: u64,
    ticks: u64,
    refusals: BTreeMap<String, u64>,
    compared: u64,
    most_trains: usize,
}

fn refusal_kind(e: &cabin_sim::interlock::Refusal) -> String {
    format!("{e:?}").split([' ', '{']).next().unwrap().to_string()
}

fn fuzz(seed: u64, n: u64) -> Counts {
    let mut rng = Rng::new(seed);
    let mut c = Counts::default();
    let mut w = world();
    let mut ck = Checker::new(&w);
    let mut set: Vec<RouteId> = Vec::new();
    let mut refused_n = 0u64;
    for i in 0..n {
        if i % EPISODE == 0 {
            w = world();
            ck = Checker::new(&w);
            set.clear();
        }
        let roll = rng.below(100);
        let action = match roll {
            0..40 => {
                let waiting: Vec<usize> = w.trains.values().filter_map(|t| t.next_signal).collect();
                let routes = w.lock.routes();
                let r = if rng.below(2) == 0 && !waiting.is_empty() {
                    let s = waiting[rng.below(waiting.len())];
                    let from: Vec<&cabin_sim::route::Route> = routes.iter().filter(|r| r.entry == s).collect();
                    if from.is_empty() { &routes[rng.below(routes.len())] } else { from[rng.below(from.len())] }
                } else {
                    &routes[rng.below(routes.len())]
                };
                Some(Action::SetRoute { entry: r.entry, exit: r.exit })
            }
            40..55 => {
                let r = if set.is_empty() || rng.below(10) == 0 { rng.below(w.lock.routes().len()) } else { set[rng.below(set.len())] };
                Some(Action::CancelRoute(r))
            }
            55..65 => {
                let p = rng.below(w.yard.portals.len());
                if w.spawn(p, LENS[rng.below(LENS.len())]).is_ok() {
                    c.offered += 1;
                }
                None
            }
            _ => {
                for _ in 0..1 + rng.below(10) {
                    let before = w.trains.len();
                    w.step_ticks(1);
                    ck.check(&w);
                    c.ticks += 1;
                    c.left += (before - w.trains.len()) as u64;
                }
                c.most_trains = c.most_trains.max(w.trains.len());
                None
            }
        };
        let Some(a) = action else { continue };
        let copy = (refused_n % 50 == 0).then(|| w.clone());
        match w.apply(a) {
            Ok(()) => match a {
                Action::SetRoute { entry, exit } => {
                    c.set += 1;
                    let id = w.lock.routes().iter().position(|r| r.entry == entry && r.exit == exit).unwrap();
                    set.push(id);
                    if set.len() > 50 {
                        set.remove(0);
                    }
                }
                Action::CancelRoute(_) => c.cancelled += 1,
            },
            Err(e) => {
                *c.refusals.entry(refusal_kind(&e)).or_insert(0) += 1;
                if let Some(before) = copy {
                    assert!(w == before, "seed {seed}, action {i}: a refused {a:?} changed the world");
                    c.compared += 1;
                }
                refused_n += 1;
            }
        }
    }
    c
}

#[test]
fn fuzz_1_000_000_random_actions_on_3_seeds_with_no_invariant_failure() {
    let per = ACTIONS / SEEDS.len() as u64;
    let mut ticks = 0;
    for (k, seed) in SEEDS.into_iter().enumerate() {
        // The last seed takes the remainder, so the total is exactly ACTIONS.
        let n = if k + 1 == SEEDS.len() { ACTIONS - per * k as u64 } else { per };
        let c = fuzz(seed, n);
        println!(
            "seed {seed}: {n} actions; {} routes set, {} cancelled, {} trains offered, {} left; {} ticks checked; most trains at once {}; {} refusals compared unchanged",
            c.set, c.cancelled, c.offered, c.left, c.ticks, c.most_trains, c.compared
        );
        println!("  refusals {:?}", c.refusals);
        assert!(c.set > 0 && c.left > 0, "seed {seed}: the fuzz moved no train through");
        ticks += c.ticks;
    }
    println!("total: {ACTIONS} actions, {ticks} ticks checked, 0 invariant failures");
}
