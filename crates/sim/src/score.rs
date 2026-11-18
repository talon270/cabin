//! CABIN · SCORE
//!   · Score: three plain numbers, never blended (plan Phase 0), approved by
//!     you on 2026-11-18
//!   · handled: trains that left by a portal during the shift
//!   · delay: game time trains were held: offered and not yet accepted, or
//!     stopped at a red Home, or stopped at a red Starter after their dwell.
//!     Dwell is not delay. A train slowing for a red signal is not counted
//!     until it stands; that is the claim, "held at red", not lateness
//!     against a timetable. Shown per train due: the total over the trains
//!     offered so far in the shift, so one ignored offer weighs as one
//!     train's wait and not as the whole score
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
    /// Trains offered at a portal so far in the shift: the delay's divisor.
    pub due: u32,
    /// The tick the shift ended on.
    pub over: Option<u64>,
}

impl Score {
    /// Delay in game minutes.
    pub fn delay_min(&self) -> f64 {
        self.held_ticks as f64 / f64::from(TICK_HZ) / 60.0
    }

    /// Delay in game minutes per train due; 0 before the first is due.
    pub fn delay_per_train(&self) -> f64 {
        if self.due == 0 { 0.0 } else { self.delay_min() / f64::from(self.due) }
    }
}
