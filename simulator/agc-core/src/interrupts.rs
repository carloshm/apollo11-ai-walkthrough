//! Interrupt subsystem for the AGC.
//!
//! The AGC has 11 interrupt vectors starting at address 04000.
//! Each vector occupies 4 words (enough for a TC to an ISR).
//!
//! Interrupts are disabled by INHINT and enabled by RELINT.
//! The ISR must manually save/restore A, L, Q, BB via shadow registers.
//! Return from interrupt is via RESUME (INDEX 25).

/// Interrupt vector addresses (in fixed-fixed memory)
pub mod vector {
    /// Boot / power-up / GOJAM
    pub const BOOT: u16 = 0o4000;
    /// T6RUPT — DAP jet control (TIME6 overflow)
    pub const T6RUPT: u16 = 0o4004;
    /// T5RUPT — Autopilot loop (TIME5 overflow)
    pub const T5RUPT: u16 = 0o4010;
    /// T3RUPT — Waitlist task scheduler (TIME3 overflow)
    pub const T3RUPT: u16 = 0o4014;
    /// T4RUPT — DSKY display update (TIME4 overflow)
    pub const T4RUPT: u16 = 0o4020;
    /// KEYRUPT1 — Primary DSKY keyboard
    pub const KEYRUPT1: u16 = 0o4024;
    /// KEYRUPT2 — Navigator DSKY (CM only)
    pub const KEYRUPT2: u16 = 0o4030;
    /// UPRUPT — Ground uplink data ready
    pub const UPRUPT: u16 = 0o4034;
    /// DOWNRUPT — Telemetry downlink ready
    pub const DOWNRUPT: u16 = 0o4040;
    /// RADARRUPT — Radar data ready
    pub const RADARRUPT: u16 = 0o4044;
    /// RUPT10 — Hand controller input
    pub const RUPT10: u16 = 0o4050;
}

/// Interrupt identifiers (index into pending array)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Interrupt {
    T6Rupt = 0,
    T5Rupt = 1,
    T3Rupt = 2,
    T4Rupt = 3,
    KeyRupt1 = 4,
    KeyRupt2 = 5,
    UpRupt = 6,
    DownRupt = 7,
    RadarRupt = 8,
    Rupt10 = 9,
}

impl Interrupt {
    /// Get the vector address for this interrupt
    pub fn vector_address(self) -> u16 {
        match self {
            Interrupt::T6Rupt => vector::T6RUPT,
            Interrupt::T5Rupt => vector::T5RUPT,
            Interrupt::T3Rupt => vector::T3RUPT,
            Interrupt::T4Rupt => vector::T4RUPT,
            Interrupt::KeyRupt1 => vector::KEYRUPT1,
            Interrupt::KeyRupt2 => vector::KEYRUPT2,
            Interrupt::UpRupt => vector::UPRUPT,
            Interrupt::DownRupt => vector::DOWNRUPT,
            Interrupt::RadarRupt => vector::RADARRUPT,
            Interrupt::Rupt10 => vector::RUPT10,
        }
    }
}

/// Number of interrupt sources
pub const NUM_INTERRUPTS: usize = 10;

/// All interrupt types in priority order (T6RUPT is highest priority)
pub const INTERRUPT_PRIORITY: [Interrupt; NUM_INTERRUPTS] = [
    Interrupt::T6Rupt,
    Interrupt::T5Rupt,
    Interrupt::T3Rupt,
    Interrupt::T4Rupt,
    Interrupt::KeyRupt1,
    Interrupt::KeyRupt2,
    Interrupt::UpRupt,
    Interrupt::DownRupt,
    Interrupt::RadarRupt,
    Interrupt::Rupt10,
];

/// Interrupt controller state
#[derive(Clone)]
pub struct InterruptController {
    /// Pending interrupt flags (one per interrupt source)
    pub pending: [bool; NUM_INTERRUPTS],
    /// Whether interrupts are globally enabled
    pub enabled: bool,
    /// Whether we are currently in an ISR
    pub in_isr: bool,
    /// Shadow registers for ISR context save
    pub arupt: u16,
    pub lrupt: u16,
    pub qrupt: u16,
    pub bbrupt: u16,
    pub zrupt: u16,
}

impl InterruptController {
    pub fn new() -> Self {
        Self {
            pending: [false; NUM_INTERRUPTS],
            enabled: true,
            in_isr: false,
            arupt: 0,
            lrupt: 0,
            qrupt: 0,
            bbrupt: 0,
            zrupt: 0,
        }
    }

    /// Request an interrupt
    pub fn request(&mut self, interrupt: Interrupt) {
        self.pending[interrupt as usize] = true;
    }

    /// Clear a pending interrupt
    pub fn clear(&mut self, interrupt: Interrupt) {
        self.pending[interrupt as usize] = false;
    }

    /// Check if any interrupt is pending and can be serviced
    pub fn poll(&self) -> Option<Interrupt> {
        if !self.enabled || self.in_isr {
            return None;
        }
        for &irq in &INTERRUPT_PRIORITY {
            if self.pending[irq as usize] {
                return Some(irq);
            }
        }
        None
    }

    /// Disable interrupts (INHINT)
    pub fn inhibit(&mut self) {
        self.enabled = false;
    }

    /// Enable interrupts (RELINT)
    pub fn release(&mut self) {
        self.enabled = true;
    }

    /// Enter ISR: save context and disable further interrupts
    pub fn enter_isr(&mut self, a: u16, l: u16, q: u16, bb: u16, z: u16) {
        self.arupt = a;
        self.lrupt = l;
        self.qrupt = q;
        self.bbrupt = bb;
        self.zrupt = z;
        self.in_isr = true;
    }

    /// Exit ISR (RESUME): restore context
    pub fn exit_isr(&mut self) -> (u16, u16, u16, u16, u16) {
        self.in_isr = false;
        self.enabled = true;
        (self.arupt, self.lrupt, self.qrupt, self.bbrupt, self.zrupt)
    }
}

impl Default for InterruptController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interrupt_request_and_poll() {
        let mut ic = InterruptController::new();
        assert!(ic.poll().is_none());
        ic.request(Interrupt::T3Rupt);
        assert_eq!(ic.poll(), Some(Interrupt::T3Rupt));
    }

    #[test]
    fn test_interrupt_priority() {
        let mut ic = InterruptController::new();
        ic.request(Interrupt::T3Rupt);
        ic.request(Interrupt::T6Rupt);
        // T6 has higher priority
        assert_eq!(ic.poll(), Some(Interrupt::T6Rupt));
    }

    #[test]
    fn test_inhibit() {
        let mut ic = InterruptController::new();
        ic.request(Interrupt::T3Rupt);
        ic.inhibit();
        assert!(ic.poll().is_none());
        ic.release();
        assert_eq!(ic.poll(), Some(Interrupt::T3Rupt));
    }

    #[test]
    fn test_isr_blocks_nested() {
        let mut ic = InterruptController::new();
        ic.request(Interrupt::T3Rupt);
        ic.enter_isr(0, 0, 0, 0, 0);
        // While in ISR, new interrupts should not be polled
        ic.request(Interrupt::T6Rupt);
        assert!(ic.poll().is_none());
        ic.exit_isr();
        // After exit, should see T6 (higher priority)
        assert_eq!(ic.poll(), Some(Interrupt::T6Rupt));
    }
}
