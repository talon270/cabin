//! B5 soak: 10 game hours of busy-day traffic on each of 3 seeds, worked by
//! the auto-signaller, with the invariant checker after every tick. Then the
//! arrivals stop and the yard must drain: a train left behind is a deadlock.
//!
//! The traffic here is a stand-in for B7's `Schedule`, read by hand from
//! `data/ndls.traffic.toml`: arrivals at the busy day's hourly rate from
//! 06:00, a third from the north, the class mix, rake lengths and dwells.
//! Every train arrives, dwells and departs; there are no originating or
//! terminating trains, so the decision count is one arrival and one
//! departure per train. An offered train waits outside until the
//! signaller can route it into a berth (`Signaller::admit`); the longest
//! such wait is printed.
//!
//! Prints what the plan asks B5 for: decisions per real minute at
//! TIME_SCALE 1, 4 and 10, and how often the shortest route was blocked
//! while another path to the same exit was free.
//!
//! Takes about 12 s in release (11.4 s on 2026-11-17); run with
//! `cargo test --release -p cabin-sim --test b5_long_run -- --nocapture`.

mod common;

use std::collections::{BTreeMap, VecDeque};

use cabin_sim::consts::TICK_HZ;
use cabin_sim::route::PortalId;
use cabin_sim::{Rng, World};
use common::invariant::Checker;
use common::signaller::Signaller;
use common::world;

/// Busy-day trains per hour from 00:00: `hourly_weights`, V-12, secondary.
const HOURLY: [u32; 24] = [3, 2, 0, 2, 5, 9, 20, 16, 14, 22, 19, 10, 13, 14, 9, 7, 14, 21, 18, 12, 10, 12, 10, 13];
/// The soak runs 06:00 to 16:00: the morning peak and the midday.
const START_HOUR: u64 = 6;
const SOAK_H: u64 = 10;
const SEEDS: [u64; 3] = [1, 2, 3];
/// `north_share`, assumed.
const NORTH_SHARE: f64 = 0.333;
/// No route set for this long, with trains in the yard: a deadlock. The
/// longest dwell is 1500 s, so a full yard of dwelling trains stays under it.
const STALL_LIMIT_S: u64 = 3600;
/// After the arrivals stop, every train must leave within this.
const DRAIN_LIMIT_S: u64 = 3 * 3600;
const SCALES: [f64; 3] = [1.0, 4.0, 10.0];

/// (share, rake + loco in metres, dwell low and high in seconds), from
/// `[mix]`, `[length_m]` and `[dwell_s]`; the loco, 20.6 m, is added to
/// every loco-hauled train. All assumed or secondary in the profile.
const MIX: [(f64, f64, u64, u64); 5] = [
    (0.28, 264.0, 120, 600),   // EMU local, through_emu dwell
    (0.41, 530.6, 900, 1500),  // express: superfast / mail_express
    (0.148, 548.6, 900, 1500), // rajdhani
    (0.116, 404.6, 900, 1500), // shatabdi
    (0.046, 476.6, 900, 1500), // duronto
];

struct Offer {
    len_m: f64,
    dwell_s: u64,
    since: u64,
}

struct Soak {
    w: World,
    ck: Checker,
    sg: Signaller,
    queues: BTreeMap<PortalId, VecDeque<Offer>>,
    offered: u64,
    entered: u64,
    max_entry_wait_s: u64,
}

impl Soak {
    fn tick(&mut self) {
        let now = self.w.tick;
        for (&p, q) in self.queues.iter_mut() {
            if let Some(o) = q.front()
                && self.sg.admit(&mut self.w, p, o.len_m, o.dwell_s).is_some()
            {
                self.max_entry_wait_s = self.max_entry_wait_s.max((now - o.since) / TICK_HZ as u64);
                self.entered += 1;
                q.pop_front();
            }
        }
        self.w.step_ticks(1);
        self.ck.check(&self.w);
        self.sg.step(&mut self.w);
        let stalled = self.sg.stalled_s(&self.w);
        assert!(stalled < STALL_LIMIT_S, "deadlock at tick {}: no route set for {stalled} s with {} trains in the yard:{}", self.w.tick, self.w.trains.len(), self.sg.explain(&self.w));
    }
}

fn soak(seed: u64) -> Soak {
    let w = world();
    let mut s = Soak { ck: Checker::new(&w), sg: Signaller::new(&w), w, queues: BTreeMap::new(), offered: 0, entered: 0, max_entry_wait_s: 0 };
    let (north, south): (Vec<PortalId>, Vec<PortalId>) = (0..s.w.yard.portals.len()).partition(|&p| s.w.yard.portals[p].name.starts_with('N'));
    let mut rng = Rng::new(seed);
    let hour_ticks = 3600 * TICK_HZ as u64;
    for t in 0..SOAK_H * hour_ticks {
        let rate = HOURLY[((START_HOUR + t / hour_ticks) % 24) as usize] as f64;
        if rng.next_f64() < rate / hour_ticks as f64 {
            let mut x = rng.next_f64();
            let &(_, len_m, lo, hi) = MIX.iter().find(|m| {
                x -= m.0;
                x < 0.0
            }).unwrap_or(&MIX[0]);
            let side = if rng.next_f64() < NORTH_SHARE { &north } else { &south };
            let p = side[rng.below(side.len())];
            let dwell_s = lo + rng.below((hi - lo + 1) as usize) as u64;
            s.queues.entry(p).or_default().push_back(Offer { len_m, dwell_s, since: s.w.tick });
            s.offered += 1;
        }
        s.tick();
    }
    let drain_from = s.w.tick;
    while !(s.w.trains.is_empty() && s.queues.values().all(VecDeque::is_empty)) {
        let waited = (s.w.tick - drain_from) / TICK_HZ as u64;
        let left: Vec<String> = s.w.trains.values().map(|t| format!("{} at {:?}", t.id, t.next_signal.map(|x| &s.w.yard.signals[x].name))).collect();
        assert!(waited < DRAIN_LIMIT_S, "seed {seed}: {} trains still in the yard {waited} s after the arrivals stopped: {left:?}", s.w.trains.len());
        s.tick();
    }
    s
}

#[test]
fn soak_10_game_hours_on_3_seeds_with_no_invariant_failure_and_no_deadlock() {
    let mut total_decisions = 0;
    let mut peak_hour = 0;
    let (mut blocked, mut alt) = (0, 0);
    for seed in SEEDS {
        let s = soak(seed);
        let sg = &s.sg;
        let hours = (s.w.tick as f64) / (3600.0 * TICK_HZ as f64);
        println!(
            "seed {seed}: {} trains offered, {} entered, all left; {} ticks checked ({hours:.1} game h incl. drain); {} decisions, peak hour {}",
            s.offered,
            s.entered,
            s.ck.ticks,
            sg.decisions(),
            sg.per_hour.iter().max().unwrap_or(&0)
        );
        println!(
            "  refusals {:?}; Occupied at a junction by a train at a Starter: {}; path blocked {} of which another path free {}",
            sg.refusals, sg.fouled_by_berthed, sg.path_blocked, sg.alt_free
        );
        println!(
            "  longest waits: at a Home {} s, at a Starter after dwell {} s ({}), outside a portal {} s",
            sg.max_wait_home_s,
            sg.max_wait_starter_s,
            sg.max_wait_starter.map_or("-", |x| &s.w.yard.signals[x].name),
            s.max_entry_wait_s
        );
        total_decisions += sg.decisions();
        // Only the soak's hours count towards the peak, not the drain.
        peak_hour = peak_hour.max(*sg.per_hour.iter().take(SOAK_H as usize).max().unwrap_or(&0));
        blocked += sg.path_blocked;
        alt += sg.alt_free;
        assert_eq!(s.entered, s.offered);
    }
    let per_game_min = total_decisions as f64 / (SEEDS.len() as f64 * SOAK_H as f64 * 60.0);
    for k in SCALES {
        println!("TIME_SCALE {k:>2}: {:.2} decisions per real minute on average, {:.2} in the peak hour", per_game_min * k, peak_hour as f64 / 60.0 * k);
    }
    println!("shortest route's path blocked while another path to the same exit was free: {alt} of {blocked} refusals ({:.1}%)", 100.0 * alt as f64 / blocked.max(1) as f64);
}

#[test]
fn the_same_seed_gives_the_same_run() {
    // One game hour is enough to diverge if anything reads an unordered
    // collection or a clock.
    let run = |seed| {
        let w = world();
        let mut s = Soak { ck: Checker::new(&w), sg: Signaller::new(&w), w, queues: BTreeMap::new(), offered: 0, entered: 0, max_entry_wait_s: 0 };
        let mut rng = Rng::new(seed);
        for _ in 0..3600 * TICK_HZ {
            if rng.next_f64() < 0.01 {
                let p = rng.below(s.w.yard.portals.len());
                s.queues.entry(p).or_default().push_back(Offer { len_m: 400.0, dwell_s: 300, since: s.w.tick });
            }
            s.tick();
        }
        (s.w, s.sg.decisions())
    };
    let (a, da) = run(7);
    let (b, db) = run(7);
    assert!(da > 0, "the run set no route");
    assert_eq!(da, db);
    assert!(a == b, "two runs of seed 7 ended in different worlds");
}
