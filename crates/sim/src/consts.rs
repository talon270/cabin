//! CABIN · CONSTANTS
//!   · every tunable number of the game, each tagged `solid` (a cited rule
//!     or a measurement) or `assumed` (authored, to be checked)
//!
//! The first line of each constant's doc comment carries the tag;
//! `tests/consts_tagged.rs` fails the build on a missing one. A cited ledger
//! row is written `C3-143 "phrase"`; `tests/ledger.rs` fails if that row of
//! `rails/research.md` no longer holds the phrase.

/// assumed: how far the panel stretches the cross-track direction (plan A5).
/// NDLS's 16 platform tracks sit in 115 m across, so at true scale
/// neighbouring signals are a few pixels apart. Real control panels are
/// schematic for the same reason. You kept 6 on 2026-11-17 and chose a
/// closer default view instead; B6's 44 px check runs at that view.
pub const PANEL_STRETCH: f64 = 6.0;

/// solid: the line beyond a signal that must be clear before the signal in
/// rear clears, metres: C3-143 "120 m with multiple-aspect". That NDLS has
/// multiple-aspect signals is assumed; with two-aspect signals it is 180 m.
pub const OVERLAP_M: f64 = 120.0;

/// assumed: how far a generated Home stands before the first connection on
/// its line, metres. The rule places it outside all connections
/// (C3-58 "outside all connections"); the distance is authored.
pub const HOME_SETBACK_M: f64 = 30.0;

/// assumed: how far a generated Starter stands before the first facing
/// points or fouling mark ahead of its platform, metres
/// (C3-63 "the first facing points"); the distance is authored.
pub const STARTER_SETBACK_M: f64 = 10.0;

/// assumed: sim ticks per second of game time. A train at 50 km/h moves
/// 6.9 m a tick, well under the 30 m fouling length, so no train crosses a
/// fouling mark between two checks.
pub const TICK_HZ: u32 = 2;

/// assumed: line speed through the station, km/h. C3-52 "not exceeding 50 km/h" is
/// Southern Railway's figure for facing points set normal; NDLS is on
/// Northern Railway, whose Subsidiary Rules were not read.
pub const SPEED_NORMAL_KMH: f64 = 50.0;

/// assumed: speed over points set reverse, km/h. C3-49 "15 km/h over facing points" is
/// Southern Railway's figure.
pub const SPEED_REVERSE_KMH: f64 = 15.0;

/// assumed: how long a route cancelled with a train on its approach stays
/// locked, seconds. No ledger row gives the time.
pub const APPROACH_RELEASE_S: u32 = 120;

/// assumed: how close to a junction a train may stand before it fouls
/// another track there, metres. C1-01 defines the fouling mark and gives no
/// distance; T-40 places it where track centres close to 4.265 m. Measuring
/// that per junction from the track polylines would replace this figure.
pub const FOULING_M: f64 = 30.0;

/// assumed: the time after a train leaves by a portal before another may
/// be sent the same way, seconds. It stands in for Line Clear from the
/// block station in advance (C3-148 "Line Clear").
pub const BLOCK_HEADWAY_S: u32 = 180;

/// assumed: game seconds per real second. Chosen by you on 2026-11-17 from
/// the B5 soak: at 4 the busy day asks about 2.0 route settings per real
/// minute on average and 2.6 in the peak hour.
pub const TIME_SCALE: f64 = 4.0;

/// assumed: the game clock when a shift begins, seconds after midnight:
/// 06:30. Chosen by you on 2026-11-18 so a shift runs into the 07:00 hour,
/// the busy day's second-busiest morning hour (20 trains, V-12).
pub const SHIFT_START_S: u32 = 6 * 3600 + 1800;

/// assumed: a scored shift's length in game seconds: 90 minutes, which is
/// 22.5 real minutes at TIME_SCALE 4. Chosen by you on 2026-11-18. Fixed, so
/// bests compare like with like.
pub const SHIFT_LEN_S: u32 = 90 * 60;

/// assumed: Light's share of the busy day's hourly rate (V-12).
pub const LIGHT_FRACTION: f64 = 0.5;

/// assumed: Normal's share of the busy day's hourly rate (V-12).
pub const NORMAL_FRACTION: f64 = 0.75;

/// assumed: Peak's share of the busy day's hourly rate (V-12): all of it.
pub const PEAK_FRACTION: f64 = 1.0;

/// assumed: the fewest trains Practice offers (plan B7: 2 to 6, no score).
pub const PRACTICE_MIN: u32 = 2;

/// assumed: the most trains Practice offers.
pub const PRACTICE_MAX: u32 = 6;

/// assumed: Practice offers its trains within this many game seconds of
/// the start: 30 minutes, 7.5 real minutes.
pub const PRACTICE_SPREAD_S: u32 = 30 * 60;

/// assumed: train acceleration, m/s².
pub const ACCEL_MS2: f64 = 0.3;

/// assumed: train service braking, m/s².
pub const BRAKE_MS2: f64 = 0.5;
