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
