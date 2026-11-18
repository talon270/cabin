//! CABIN · TRAFFIC
//!   · Profile: data/ndls.traffic, baked by cabin-osm from the sourced
//!     data/ndls.traffic.toml; hourly rates, sides, class mix, dwells, pool
//!   · Mode: Practice (2 to 6 trains, no score), Light, Normal, Peak
//!   · schedule: one shift's trains, generated whole from the seed
//!
//! The schedule is a generated timetable, not a Poisson stream. Each hour
//! gets the busy day's count for that hour times the mode's fraction,
//! rounded on the running total so no fraction of a train is lost, at
//! times drawn within the hour. So a shift's train count is the profile's,
//! not a draw around it, and delay can be measured against a due time.
//! It draws from its own `Rng`, so nothing done in play changes it.
//!
//! Every train arrives at a portal, dwells and leaves by a portal: there
//! is no shunting (plan, out of scope), so an originating train comes in
//! as an empty rake and a terminating one leaves as one. Through or not
//! sets only the dwell, and its share comes from the volume ledger, not
//! the pool (research/traffic.md).

use crate::Rng;
use crate::consts::{LIGHT_FRACTION, NORMAL_FRACTION, PEAK_FRACTION, PRACTICE_MAX, PRACTICE_MIN, PRACTICE_SPREAD_S};

/// The profile the game ships with, baked into the build.
pub const NDLS_TRAFFIC: &str = include_str!("../../../data/ndls.traffic");

#[derive(Clone, Debug, PartialEq)]
pub struct Class {
    pub name: String,
    pub share: f64,
    /// A through train's dwell, low and high, game seconds.
    pub dwell: (u32, u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PoolTrain {
    pub number: String,
    pub class: usize,
    /// Rake and loco, metres.
    pub len_m: f64,
    pub through: bool,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    /// The date the traffic ledgers were read, YYYY-MM-DD.
    pub written: String,
    /// Busy-day trains in each hour from 00:00.
    pub hourly: [u32; 24],
    pub north_share: f64,
    pub through_share: f64,
    /// Dwell of a train that starts or ends here, low and high.
    pub terminating: (u32, u32),
    pub classes: Vec<Class>,
    pub pool: Vec<PoolTrain>,
}

impl Profile {
    pub fn parse(text: &str) -> Result<Profile, String> {
        let mut p = Profile { written: String::new(), hourly: [0; 24], north_share: -1.0, through_share: -1.0, terminating: (0, 0), classes: Vec::new(), pool: Vec::new() };
        let mut pending: Vec<(usize, Vec<&str>, String)> = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let bad = |what: &str| format!("ndls.traffic line {}: {what}", i + 1);
            let f: Vec<&str> = line.split_whitespace().collect();
            let n = |k: usize| f.get(k).and_then(|x| x.parse::<f64>().ok()).ok_or_else(|| bad("expected a number"));
            let u = |k: usize| f.get(k).and_then(|x| x.parse::<u32>().ok()).ok_or_else(|| bad("expected a whole number"));
            match f.first().copied() {
                None => {}
                Some(c) if c.starts_with('#') => {}
                Some("hourly") if f.len() == 25 => {
                    for h in 0..24 {
                        p.hourly[h] = u(h + 1)?;
                    }
                }
                Some("written") if f.len() == 2 => p.written = f[1].to_string(),
                Some("north_share") => p.north_share = n(1)?,
                Some("through_share") => p.through_share = n(1)?,
                Some("terminating") => p.terminating = (u(1)?, u(2)?),
                Some("class") if f.len() == 5 => p.classes.push(Class { name: f[1].to_string(), share: n(2)?, dwell: (u(3)?, u(4)?) }),
                // Classes come first in the file, but resolve after the loop
                // so the order of lines does not matter.
                Some("train") if f.len() >= 6 => pending.push((i, f[..5].to_vec(), f[5..].join(" "))),
                Some(_) => return Err(bad("not a line this format knows")),
            }
        }
        for (i, f, name) in pending {
            let bad = |what: &str| format!("ndls.traffic line {}: {what}", i + 1);
            let class = p.classes.iter().position(|c| c.name == f[2]).ok_or_else(|| bad("unknown class"))?;
            let len_m = f[3].parse::<f64>().map_err(|_| bad("bad length"))?;
            p.pool.push(PoolTrain { number: f[1].to_string(), class, len_m, through: f[4] == "through", name });
        }
        let unit = |x: f64| (0.0..=1.0).contains(&x);
        if !unit(p.north_share) || !unit(p.through_share) || p.terminating.0 == 0 || p.written.is_empty() {
            return Err("ndls.traffic: written, north_share, through_share or terminating is missing".into());
        }
        let sum: f64 = p.classes.iter().map(|c| c.share).sum();
        if (sum - 1.0).abs() > 1e-9 {
            return Err(format!("ndls.traffic: class shares sum to {sum}"));
        }
        // A class that can be drawn must have a train to draw.
        if let Some(c) = p.classes.iter().enumerate().find(|(k, c)| c.share > 0.0 && !p.pool.iter().any(|t| t.class == *k)) {
            return Err(format!("ndls.traffic: class {} has a share and no train", c.1.name));
        }
        Ok(p)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mode {
    Practice,
    Light,
    Normal,
    Peak,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::Practice, Mode::Light, Mode::Normal, Mode::Peak];

    pub fn name(self) -> &'static str {
        match self {
            Mode::Practice => "practice",
            Mode::Light => "light",
            Mode::Normal => "normal",
            Mode::Peak => "peak",
        }
    }

    pub fn parse(s: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.name() == s)
    }

    /// The share of the busy day's hourly rate; Practice has none.
    pub fn fraction(self) -> f64 {
        match self {
            Mode::Practice => 0.0,
            Mode::Light => LIGHT_FRACTION,
            Mode::Normal => NORMAL_FRACTION,
            Mode::Peak => PEAK_FRACTION,
        }
    }

    pub fn scored(self) -> bool {
        self != Mode::Practice
    }
}

/// One train of the schedule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Due {
    /// Game seconds after midnight.
    pub at_s: u32,
    pub north: bool,
    /// Index into `Profile::pool`.
    pub train: usize,
    pub dwell_s: u32,
}

/// A distinct stream from any other drawn from the same seed.
const SALT: u64 = 0x7A4F_F1C0_5EED_C4B1;

/// The trains due between `start_s` and `start_s + len_s` (game seconds
/// after midnight), in time order. The first is moved to `start_s`, so a
/// shift never opens on an empty panel. Same arguments, same schedule.
pub fn schedule(p: &Profile, mode: Mode, seed: u64, start_s: u32, len_s: u32) -> Vec<Due> {
    let mut rng = Rng::new(seed ^ SALT);
    let end_s = start_s + len_s;
    let mut times: Vec<u32> = Vec::new();
    if mode == Mode::Practice {
        let n = PRACTICE_MIN + rng.below((PRACTICE_MAX - PRACTICE_MIN + 1) as usize) as u32;
        for _ in 0..n {
            times.push(start_s + rng.below(PRACTICE_SPREAD_S.min(len_s) as usize) as u32);
        }
    } else {
        let mut total = 0.0_f64;
        for h in start_s / 3600..end_s.div_ceil(3600) {
            let w = f64::from(p.hourly[(h % 24) as usize]) * mode.fraction();
            let n = ((total + w).round() - total.round()) as u32;
            total += w;
            for _ in 0..n {
                let t = h * 3600 + rng.below(3600) as u32;
                // Drawn for the whole hour either way, so the times inside
                // the window do not depend on where the window cuts.
                if (start_s..end_s).contains(&t) {
                    times.push(t);
                }
            }
        }
    }
    times.sort_unstable();
    if let Some(first) = times.first_mut() {
        *first = start_s;
    }
    let mut used = vec![false; p.pool.len()];
    times.into_iter().map(|at_s| draw(p, &mut rng, &mut used, at_s)).collect()
}

/// Class by share; through by the volume share; then a train of that class,
/// one that matches through or not if any is left, never one already used
/// this shift while the class has another.
fn draw(p: &Profile, rng: &mut Rng, used: &mut [bool], at_s: u32) -> Due {
    let mut x = rng.next_f64();
    let class = p.classes.iter().position(|c| {
        x -= c.share;
        x < 0.0
    });
    // Rounding can leave x a hair above 0 after the last class.
    let class = class.unwrap_or_else(|| p.classes.iter().rposition(|c| c.share > 0.0).expect("a class with a share"));
    let through = rng.next_f64() < p.through_share;
    let north = rng.next_f64() < p.north_share;
    let of_class: Vec<usize> = (0..p.pool.len()).filter(|&k| p.pool[k].class == class).collect();
    if of_class.iter().all(|&k| used[k]) {
        for &k in &of_class {
            used[k] = false;
        }
    }
    let free: Vec<usize> = of_class.iter().copied().filter(|&k| !used[k]).collect();
    let matching: Vec<usize> = free.iter().copied().filter(|&k| p.pool[k].through == through).collect();
    let from = if matching.is_empty() { &free } else { &matching };
    let train = from[rng.below(from.len())];
    used[train] = true;
    let (lo, hi) = if through { p.classes[class].dwell } else { p.terminating };
    let dwell_s = lo + rng.below((hi - lo + 1) as usize) as u32;
    Due { at_s, north, train, dwell_s }
}
