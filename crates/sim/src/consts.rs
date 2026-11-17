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
/// schematic for the same reason. 6 is the starting value; B6 sets it from
/// the 44 px hit-slop check.
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

/// assumed: train acceleration, m/s².
pub const ACCEL_MS2: f64 = 0.3;

/// assumed: train service braking, m/s².
pub const BRAKE_MS2: f64 = 0.5;
