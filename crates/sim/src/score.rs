//! CABIN · SCORE
//!   · Score: three plain numbers, never blended (plan Phase 0), approved by
//!     you on 2026-11-18
//!   · handled: trains that left by a portal during the shift
//!   · delay: game time trains were held: offered and not yet accepted, or
//!     stopped at a red Home, or stopped at a red Starter after their dwell.
//!     Dwell is not delay. A train slowing for a red signal is not counted
//!     until it stands; that is the claim, "held at red", not lateness
//!     against a timetable
//!   · put-backs: routes cancelled after a train took them, or while a
//!     train was on the approach to the entry signal
//!
//! Practice keeps no score. A shift is over at SHIFT_LEN_S; the score stops
//! counting then and the world plays on.

use crate::consts::TICK_HZ;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Score {
    pub handled: u32,
    /// Ticks, summed over trains, spent held.
    pub held_ticks: u64,
    pub put_backs: u32,
    /// The tick the shift ended on.
    pub over: Option<u64>,
}

impl Score {
    /// Delay in game minutes.
    pub fn delay_min(&self) -> f64 {
        self.held_ticks as f64 / f64::from(TICK_HZ) / 60.0
    }
}
