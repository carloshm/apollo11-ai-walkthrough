//! Timer/counter subsystem for the AGC.
//!
//! The AGC has several hardware timers that increment automatically:
//! - TIME1: Master clock, incremented every 10 ms
//! - TIME2: Incremented on TIME1 overflow (together = 28-bit clock)
//! - TIME3: 10 ms counter; overflow → T3RUPT
//! - TIME4: 10 ms counter; overflow → T4RUPT
//! - TIME5: 10 ms counter; overflow → T5RUPT
//! - TIME6: 1/1600 second counter; overflow → T6RUPT (when enabled)
//!
//! Counter registers (CDUX/Y/Z, PIPAX/Y/Z) are updated by external hardware.

use crate::interrupts::Interrupt;

/// Timer register addresses in erasable memory
pub mod addr {
    pub const TIME2: usize = 0o24;
    pub const TIME1: usize = 0o25;
    pub const TIME3: usize = 0o26;
    pub const TIME4: usize = 0o27;
    pub const TIME5: usize = 0o30;
    pub const TIME6: usize = 0o31;
    pub const CDUX: usize = 0o32;
    pub const CDUY: usize = 0o33;
    pub const CDUZ: usize = 0o34;
    pub const PIPAX: usize = 0o37;
    pub const PIPAY: usize = 0o40;
    pub const PIPAZ: usize = 0o41;
}

/// MCT (Machine Cycle Time) in nanoseconds: 11,720 ns = 11.72 µs
pub const MCT_NS: u64 = 11_720;

/// 10 ms in nanoseconds
pub const TIMER_10MS_NS: u64 = 10_000_000;

/// MCTs per 10 ms timer tick (10,000,000 / 11,720 ≈ 853)
pub const MCTS_PER_10MS: u64 = TIMER_10MS_NS / MCT_NS;

/// TIME6 increment period: 1/1600 second = 625 µs = 625,000 ns
pub const TIME6_PERIOD_NS: u64 = 625_000;

/// MCTs per TIME6 tick (625,000 / 11,720 ≈ 53)
pub const MCTS_PER_TIME6: u64 = TIME6_PERIOD_NS / MCT_NS;

/// Timer subsystem state
#[derive(Clone)]
pub struct Timers {
    /// MCT counter for 10 ms timer ticks
    pub mct_accumulator_10ms: u64,
    /// MCT counter for TIME6 ticks
    pub mct_accumulator_t6: u64,
    /// Whether TIME6 is enabled (controlled by T6RUPT enable bit)
    pub time6_enabled: bool,
    /// Total MCTs executed (for simulation time tracking)
    pub total_mcts: u64,
}

impl Timers {
    pub fn new() -> Self {
        Self {
            mct_accumulator_10ms: 0,
            mct_accumulator_t6: 0,
            time6_enabled: false,
            total_mcts: 0,
        }
    }

    /// Advance timers by the given number of MCTs.
    /// Returns a list of interrupts that should be triggered.
    pub fn tick(&mut self, mcts: u64, erasable: &mut [u16]) -> Vec<Interrupt> {
        let mut interrupts = Vec::new();
        self.total_mcts += mcts;
        self.mct_accumulator_10ms += mcts;

        // Process 10 ms timer ticks
        while self.mct_accumulator_10ms >= MCTS_PER_10MS {
            self.mct_accumulator_10ms -= MCTS_PER_10MS;

            // Increment TIME1 (master clock)
            let t1 = increment_ones_complement(erasable[addr::TIME1]);
            if is_overflow(t1) {
                erasable[addr::TIME1] = 0;
                // TIME1 overflow → increment TIME2
                let t2 = increment_ones_complement(erasable[addr::TIME2]);
                erasable[addr::TIME2] = t2 & 0x7FFF;
            } else {
                erasable[addr::TIME1] = t1 & 0x7FFF;
            }

            // Increment TIME3 → T3RUPT on overflow
            let t3 = increment_ones_complement(erasable[addr::TIME3]);
            if is_overflow(t3) {
                erasable[addr::TIME3] = 0;
                interrupts.push(Interrupt::T3Rupt);
            } else {
                erasable[addr::TIME3] = t3 & 0x7FFF;
            }

            // Increment TIME4 → T4RUPT on overflow
            let t4 = increment_ones_complement(erasable[addr::TIME4]);
            if is_overflow(t4) {
                erasable[addr::TIME4] = 0;
                interrupts.push(Interrupt::T4Rupt);
            } else {
                erasable[addr::TIME4] = t4 & 0x7FFF;
            }

            // Increment TIME5 → T5RUPT on overflow
            let t5 = increment_ones_complement(erasable[addr::TIME5]);
            if is_overflow(t5) {
                erasable[addr::TIME5] = 0;
                interrupts.push(Interrupt::T5Rupt);
            } else {
                erasable[addr::TIME5] = t5 & 0x7FFF;
            }
        }

        // Process TIME6 ticks (higher frequency)
        if self.time6_enabled {
            self.mct_accumulator_t6 += mcts;
            while self.mct_accumulator_t6 >= MCTS_PER_TIME6 {
                self.mct_accumulator_t6 -= MCTS_PER_TIME6;
                let t6 = increment_ones_complement(erasable[addr::TIME6]);
                if is_overflow(t6) {
                    erasable[addr::TIME6] = 0;
                    self.time6_enabled = false;
                    interrupts.push(Interrupt::T6Rupt);
                } else {
                    erasable[addr::TIME6] = t6 & 0x7FFF;
                }
            }
        }

        interrupts
    }

    /// Get elapsed simulation time in nanoseconds
    pub fn elapsed_ns(&self) -> u64 {
        self.total_mcts * MCT_NS
    }

    /// Get elapsed simulation time in milliseconds
    pub fn elapsed_ms(&self) -> f64 {
        self.elapsed_ns() as f64 / 1_000_000.0
    }
}

/// Increment a 15-bit 1's complement value by 1.
/// Returns 16-bit value; bit 15 indicates overflow.
fn increment_ones_complement(val: u16) -> u16 {
    let v = val & 0x7FFF;
    if v == 0x3FFF {
        // Positive max → overflow
        0x8000 // overflow flag
    } else if v == 0x7FFF {
        // Negative zero (-0) → positive zero
        0x0000
    } else if v & 0x4000 != 0 {
        // Negative: adding 1 reduces magnitude
        // In 1's complement, -N is represented as ~N
        // Adding 1 to a negative: decrease magnitude toward zero
        (v + 1) & 0x7FFF
    } else {
        // Positive: simple increment
        v + 1
    }
}

/// Check if the 16-bit result has overflow (bit 15 set)
fn is_overflow(val: u16) -> bool {
    (val & 0x8000) != 0
}

impl Default for Timers {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_increment_positive() {
        assert_eq!(increment_ones_complement(0x0005), 0x0006);
    }

    #[test]
    fn test_increment_neg_zero() {
        // -0 (0x7FFF) + 1 → +0 (0x0000)
        assert_eq!(increment_ones_complement(0x7FFF), 0x0000);
    }

    #[test]
    fn test_increment_positive_max_overflow() {
        // 0x3FFF (max positive) + 1 → overflow
        assert!(is_overflow(increment_ones_complement(0x3FFF)));
    }

    #[test]
    fn test_timer_tick() {
        let mut timers = Timers::new();
        let mut erasable = [0u16; 2048];
        erasable[addr::TIME1] = 0x0000;
        erasable[addr::TIME3] = 0x0000;

        // Tick enough for one 10ms period
        let interrupts = timers.tick(MCTS_PER_10MS, &mut erasable);

        // TIME1 should have incremented
        assert_eq!(erasable[addr::TIME1], 0x0001);

        // No overflow interrupts expected from initial state
        // (TIME3/4/5 start at 0, won't overflow for many ticks)
        assert!(interrupts.iter().all(|i| *i != Interrupt::T3Rupt));
    }
}
