//! CABIN-SIM · LEVELS
//!   · the ladder below NDLS: one small real station per level, each
//!     teaching one skill (PLAN-redesign.md B3)
//!   · each level's trains are invented and say so: numbers start with
//!     `L`, which Indian Railways numbers never do, and their times,
//!     portals, rakes and dwells are authored, not researched
//!   · a level is done when every scripted train has left by a portal
//!
//! Public: `Level`, `Scripted`, `LEVELS`, `level(code)`.

/// One invented train: due `at_s` game seconds after the shift starts, at
/// the portal named `portal`.
#[derive(Clone, Copy, Debug)]
pub struct Scripted {
    pub at_s: u32,
    pub portal: &'static str,
    pub number: &'static str,
    pub name: &'static str,
    pub len_m: f64,
    pub dwell_s: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Level {
    /// The station code, as in data/<code>.yard.
    pub code: &'static str,
    pub station: &'static str,
    /// The one thing the level teaches, shown before it starts.
    pub skill: &'static str,
    pub yard: &'static str,
    pub trains: &'static [Scripted],
}

/// A local of 12 cars at about 21 m: within every ladder platform.
const LOCAL_M: f64 = 260.0;
const EXPRESS_M: f64 = 400.0;

/// Smallest first. NDLS, the fourth rung, is the shift modes.
pub const LEVELS: [Level; 3] = [
    Level {
        code: "huk",
        station: "Holambi Kalan",
        skill: "Take one train in and send it out: accept it, set its route to a platform, wait for its dwell, set its route out.",
        yard: include_str!("../../../data/huk.yard"),
        trains: &[
            Scripted { at_s: 30, portal: "N1", number: "L101", name: "Practice local 1 (invented)", len_m: LOCAL_M, dwell_s: 90 },
            Scripted { at_s: 360, portal: "S2", number: "L102", name: "Practice local 2 (invented)", len_m: LOCAL_M, dwell_s: 90 },
            Scripted { at_s: 720, portal: "N1", number: "L103", name: "Practice local 3 (invented)", len_m: LOCAL_M, dwell_s: 90 },
        ],
    },
    Level {
        code: "nur",
        station: "Narela",
        skill: "Two trains at once, one each way: keep both moving without one waiting on the other.",
        yard: include_str!("../../../data/nur.yard"),
        trains: &[
            Scripted { at_s: 30, portal: "N1", number: "L201", name: "Practice local 4 (invented)", len_m: LOCAL_M, dwell_s: 120 },
            Scripted { at_s: 90, portal: "S1", number: "L202", name: "Practice express 1 (invented)", len_m: EXPRESS_M, dwell_s: 120 },
            Scripted { at_s: 480, portal: "N2", number: "L203", name: "Practice local 5 (invented)", len_m: LOCAL_M, dwell_s: 120 },
            Scripted { at_s: 510, portal: "S2", number: "L204", name: "Practice express 2 (invented)", len_m: EXPRESS_M, dwell_s: 120 },
            Scripted { at_s: 960, portal: "N1", number: "L205", name: "Practice local 6 (invented)", len_m: LOCAL_M, dwell_s: 120 },
        ],
    },
    Level {
        code: "dee",
        station: "Delhi Sarai Rohilla",
        skill: "Choose the platform: several routes lead in from each side, and a train standing in one blocks others.",
        yard: include_str!("../../../data/dee.yard"),
        trains: &[
            Scripted { at_s: 30, portal: "E1", number: "L301", name: "Practice express 3 (invented)", len_m: EXPRESS_M, dwell_s: 300 },
            Scripted { at_s: 120, portal: "W1", number: "L302", name: "Practice express 4 (invented)", len_m: EXPRESS_M, dwell_s: 300 },
            Scripted { at_s: 360, portal: "E2", number: "L303", name: "Practice local 7 (invented)", len_m: LOCAL_M, dwell_s: 180 },
            Scripted { at_s: 480, portal: "W2", number: "L304", name: "Practice express 5 (invented)", len_m: EXPRESS_M, dwell_s: 300 },
            Scripted { at_s: 840, portal: "E4", number: "L305", name: "Practice local 8 (invented)", len_m: LOCAL_M, dwell_s: 180 },
            Scripted { at_s: 960, portal: "W3", number: "L306", name: "Practice express 6 (invented)", len_m: EXPRESS_M, dwell_s: 300 },
        ],
    },
];

pub fn level(code: &str) -> Option<&'static Level> {
    LEVELS.iter().find(|l| l.code == code)
}
