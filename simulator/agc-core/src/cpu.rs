//! AGC CPU — Main processor simulation.
//!
//! The AGC4 Block II processor:
//! - 15-bit word + 1 parity bit (parity not simulated)
//! - 1's complement arithmetic
//! - ~43,000 instructions/second at 1.024 MHz
//! - 11 interrupt vectors
//! - No hardware stack (only Q register for one return address)
//!
//! Register map (memory-mapped at addresses 0–7):
//!   0: A (Accumulator)
//!   1: L (Lower product)
//!   2: Q (Return address)
//!   3: EB (Erasable bank)
//!   4: FB (Fixed bank)
//!   5: Z (Program counter — next instruction)
//!   6: BB (Both banks)
//!   7: ZERO (hardwired to 0)
//!
//! Editing registers (20–23): CYR, SR, CYL, EDOP

use crate::instructions;
use crate::interrupts::InterruptController;
use crate::io::IoChannels;
use crate::memory::{Memory, WORD_MASK, SIGN_BIT, POS_ZERO};
use crate::timers::Timers;

/// Register addresses in erasable memory
pub mod reg {
    pub const A: usize = 0o00;
    pub const L: usize = 0o01;
    pub const Q: usize = 0o02;
    pub const EB: usize = 0o03;
    pub const FB: usize = 0o04;
    pub const Z: usize = 0o05;
    pub const BB: usize = 0o06;
    pub const ZERO: usize = 0o07;
    // Editing registers
    pub const CYR: usize = 0o20;
    pub const SR: usize = 0o21;
    pub const CYL: usize = 0o22;
    pub const EDOP: usize = 0o23;
}

/// AGC processor state
#[derive(Clone)]
pub struct Agc {
    /// Main memory subsystem
    pub memory: Memory,
    /// I/O channels
    pub io: IoChannels,
    /// Interrupt controller
    pub interrupts: InterruptController,
    /// Timer subsystem
    pub timers: Timers,
    /// Accumulator overflow bit (16th bit)
    pub a_overflow: bool,
    /// Whether the next instruction should be treated as an extracode
    pub extend: bool,
    /// INDEX value to be added to the next instruction
    pub index_value: Option<u16>,
    /// Whether the CPU is running
    pub running: bool,
    /// Total instructions executed
    pub instruction_count: u64,
    /// Pending DSKY key input (set externally, consumed by KEYRUPT)
    pub pending_key: Option<u16>,
    /// DSKY display table (11 words for 7-segment displays)
    pub dsptab: [u16; 12],
    /// Whether a GOJAM (reset) was triggered
    pub gojam: bool,
}

impl Agc {
    pub fn new() -> Self {
        let mut agc = Self {
            memory: Memory::new(),
            io: IoChannels::new(),
            interrupts: InterruptController::new(),
            timers: Timers::new(),
            a_overflow: false,
            extend: false,
            index_value: None,
            running: false,
            instruction_count: 0,
            pending_key: None,
            dsptab: [0u16; 12],
            gojam: false,
        };
        // Register 7 (ZERO) is hardwired to 0
        agc.memory.erasable[reg::ZERO] = POS_ZERO;
        agc
    }

    /// Load a program into fixed memory.
    /// `bank` is the fixed bank number (0–35).
    /// `data` is the program words.
    pub fn load_program(&mut self, bank: usize, data: &[u16]) {
        let offset = bank * crate::memory::FIXED_BANK_SIZE;
        self.memory.load_fixed(offset, data);
    }

    /// Set the program counter (Z register) to a given address.
    pub fn set_pc(&mut self, addr: u16) {
        self.set_reg_z(addr);
    }

    /// Get the current program counter value.
    pub fn pc(&self) -> u16 {
        self.reg_z()
    }

    /// Start the CPU.
    pub fn start(&mut self) {
        self.running = true;
    }

    /// Stop the CPU.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Trigger a GOJAM (hardware reset).
    pub fn gojam(&mut self) {
        self.gojam = true;
        self.extend = false;
        self.index_value = None;
        self.interrupts.inhibit();
        self.set_reg_z(crate::interrupts::vector::BOOT);
    }

    /// Execute a single instruction cycle.
    /// Returns the number of MCTs consumed.
    pub fn step(&mut self) -> u64 {
        if !self.running {
            return 0;
        }

        // Check for pending interrupts
        if let Some(irq) = self.interrupts.poll() {
            self.interrupts.clear(irq);
            // Save context to shadow registers
            let a = self.reg_a();
            let l = self.reg_l();
            let q = self.reg_q();
            let bb = self.reg_bb();
            let z = self.reg_z();
            self.interrupts.enter_isr(a, l, q, bb, z);
            // Jump to interrupt vector
            self.set_reg_z(irq.vector_address());
        }

        // Fetch instruction
        let z = self.reg_z();
        let instruction = self.read_mem(z);

        // Advance program counter
        self.set_reg_z(z + 1);

        // Apply INDEX modification if pending
        let instruction = if let Some(idx) = self.index_value.take() {
            let modified = instructions::ones_complement_add(instruction, idx);
            modified & WORD_MASK
        } else {
            instruction
        };

        // Check for EXTEND prefix (TC 6 = INDEX 5777 equivalent)
        if !self.extend && instruction == 0o00006 {
            self.extend = true;
            return 1;
        }

        // Execute instruction
        let mcts = if self.extend {
            self.extend = false;
            instructions::execute_extracode(self, instruction)
        } else {
            instructions::execute_basic(self, instruction)
        };

        self.instruction_count += 1;

        // Advance timers
        let timer_interrupts = self.timers.tick(mcts, &mut self.memory.erasable);
        for irq in timer_interrupts {
            self.interrupts.request(irq);
        }

        // Check for pending DSKY key input
        if let Some(key) = self.pending_key.take() {
            self.io.write(crate::io::channel::MNKEYIN, key);
            self.interrupts.request(crate::interrupts::Interrupt::KeyRupt1);
        }

        // Ensure ZERO register stays zero
        self.memory.erasable[reg::ZERO] = POS_ZERO;

        mcts
    }

    /// Run for a specified number of MCTs.
    /// Returns the actual number of MCTs executed.
    pub fn run_mcts(&mut self, max_mcts: u64) -> u64 {
        let mut total = 0u64;
        while total < max_mcts && self.running {
            total += self.step();
        }
        total
    }

    // ===== Register Access Methods =====

    /// Read the accumulator (A register, address 0)
    pub fn reg_a(&self) -> u16 {
        self.memory.erasable[reg::A] & WORD_MASK
    }

    /// Read the accumulator with overflow bit
    pub fn reg_a_with_overflow(&self) -> u16 {
        let val = self.memory.erasable[reg::A] & WORD_MASK;
        if self.a_overflow { val | 0x8000 } else { val }
    }

    /// Set the accumulator (clears overflow)
    pub fn set_reg_a(&mut self, val: u16) {
        self.memory.erasable[reg::A] = val & WORD_MASK;
        self.a_overflow = false;
    }

    /// Set the accumulator with possible overflow
    pub fn set_reg_a_with_overflow(&mut self, val: u16) {
        self.memory.erasable[reg::A] = val & WORD_MASK;
        self.a_overflow = (val & 0x8000) != 0;
    }

    /// Read L register
    pub fn reg_l(&self) -> u16 {
        self.memory.erasable[reg::L] & WORD_MASK
    }

    /// Set L register
    pub fn set_reg_l(&mut self, val: u16) {
        self.memory.erasable[reg::L] = val & WORD_MASK;
    }

    /// Read Q register
    pub fn reg_q(&self) -> u16 {
        self.memory.erasable[reg::Q] & WORD_MASK
    }

    /// Write Q register
    pub fn write_reg_q(&mut self, val: u16) {
        self.memory.erasable[reg::Q] = val & WORD_MASK;
    }

    /// Read EB (erasable bank) register
    pub fn reg_eb(&self) -> u16 {
        self.memory.erasable[reg::EB] & 0x07
    }

    /// Set EB register
    pub fn set_reg_eb(&mut self, val: u16) {
        self.memory.erasable[reg::EB] = val & 0x07;
        // Update BB register to match
        self.sync_bb();
    }

    /// Read FB (fixed bank) register
    pub fn reg_fb(&self) -> u16 {
        self.memory.erasable[reg::FB]
    }

    /// Set FB register
    pub fn set_reg_fb(&mut self, val: u16) {
        self.memory.erasable[reg::FB] = val & WORD_MASK;
        self.sync_bb();
    }

    /// Read Z (program counter)
    pub fn reg_z(&self) -> u16 {
        self.memory.erasable[reg::Z] & 0x0FFF
    }

    /// Set Z register
    pub fn set_reg_z(&mut self, val: u16) {
        self.memory.erasable[reg::Z] = val & 0x0FFF;
    }

    /// Read BB (both banks) register
    pub fn reg_bb(&self) -> u16 {
        self.memory.erasable[reg::BB] & WORD_MASK
    }

    /// Set BB register (updates both EB and FB)
    pub fn set_reg_bb(&mut self, val: u16) {
        self.memory.erasable[reg::BB] = val & WORD_MASK;
        // Extract EB and FB from BB
        self.memory.erasable[reg::EB] = val & 0x07;
        self.memory.erasable[reg::FB] = val & 0x7C00;
    }

    /// Synchronize BB from EB and FB
    fn sync_bb(&mut self) {
        let eb = self.memory.erasable[reg::EB] & 0x07;
        let fb = self.memory.erasable[reg::FB] & 0x7C00;
        self.memory.erasable[reg::BB] = fb | eb;
    }

    /// Read memory at the given 12-bit address (resolves bank switching)
    pub fn read_mem(&self, addr: u16) -> u16 {
        let addr12 = addr & 0x0FFF;

        // Special register reads with editing transformations
        match addr12 as usize {
            reg::A => return self.reg_a(),
            reg::L => return self.reg_l(),
            reg::Q => return self.reg_q(),
            reg::ZERO => return POS_ZERO,
            reg::CYR => return self.editing_cyr(),
            reg::SR => return self.editing_sr(),
            reg::CYL => return self.editing_cyl(),
            reg::EDOP => return self.editing_edop(),
            _ => {}
        }

        let eb = self.reg_eb();
        let fb = self.reg_fb();
        let superbank = self.io.superbank();
        self.memory.read(addr12, eb, fb, superbank)
    }

    /// Write memory at the given 12-bit address
    pub fn write_mem(&mut self, addr: u16, value: u16) {
        let addr12 = addr & 0x0FFF;

        // Special register writes
        match addr12 as usize {
            reg::A => { self.set_reg_a(value); return; }
            reg::L => { self.set_reg_l(value); return; }
            reg::Q => { self.write_reg_q(value); return; }
            reg::EB => { self.set_reg_eb(value); return; }
            reg::FB => { self.set_reg_fb(value); return; }
            reg::Z => { self.set_reg_z(value); return; }
            reg::BB => { self.set_reg_bb(value); return; }
            reg::ZERO => return, // Cannot write to ZERO
            reg::CYR | reg::SR | reg::CYL | reg::EDOP => {
                // Editing registers store the value; transformation happens on read
                let flat = addr12 as usize;
                self.memory.write_erasable(flat, value);
                return;
            }
            _ => {}
        }

        let eb = self.reg_eb();
        self.memory.write(addr12, value, eb);
    }

    // ===== Editing Register Transformations =====

    /// CYR (Cycle Right): bit 1 wraps to bit 15
    fn editing_cyr(&self) -> u16 {
        let val = self.memory.erasable[reg::CYR] & WORD_MASK;
        let bit1 = val & 1;
        ((val >> 1) | (bit1 << 14)) & WORD_MASK
    }

    /// SR (Shift Right): arithmetic shift right with sign extension
    fn editing_sr(&self) -> u16 {
        let val = self.memory.erasable[reg::SR] & WORD_MASK;
        let sign = val & SIGN_BIT;
        (sign | (val >> 1)) & WORD_MASK
    }

    /// CYL (Cycle Left): bit 15 wraps to bit 1
    fn editing_cyl(&self) -> u16 {
        let val = self.memory.erasable[reg::CYL] & WORD_MASK;
        let bit15 = (val >> 14) & 1;
        ((val << 1) | bit15) & WORD_MASK
    }

    /// EDOP: Shift right 7, zero upper 8 bits
    fn editing_edop(&self) -> u16 {
        let val = self.memory.erasable[reg::EDOP] & WORD_MASK;
        (val >> 7) & 0x007F
    }

    /// Send a key press to the DSKY (queues KEYRUPT1)
    pub fn dsky_key_press(&mut self, key_code: u16) {
        self.pending_key = Some(key_code & 0x1F);
    }

    /// Get the current state snapshot for debugging/display
    pub fn snapshot(&self) -> AgcSnapshot {
        AgcSnapshot {
            a: self.reg_a(),
            l: self.reg_l(),
            q: self.reg_q(),
            eb: self.reg_eb(),
            fb: self.reg_fb(),
            z: self.reg_z(),
            bb: self.reg_bb(),
            a_overflow: self.a_overflow,
            extend: self.extend,
            running: self.running,
            interrupts_enabled: self.interrupts.enabled,
            in_isr: self.interrupts.in_isr,
            instruction_count: self.instruction_count,
            elapsed_ms: self.timers.elapsed_ms(),
        }
    }
}

impl Default for Agc {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of AGC state for external inspection
#[derive(Debug, Clone)]
pub struct AgcSnapshot {
    pub a: u16,
    pub l: u16,
    pub q: u16,
    pub eb: u16,
    pub fb: u16,
    pub z: u16,
    pub bb: u16,
    pub a_overflow: bool,
    pub extend: bool,
    pub running: bool,
    pub interrupts_enabled: bool,
    pub in_isr: bool,
    pub instruction_count: u64,
    pub elapsed_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_agc() {
        let agc = Agc::new();
        assert_eq!(agc.reg_a(), POS_ZERO);
        assert_eq!(agc.reg_z(), 0);
        assert!(!agc.running);
    }

    #[test]
    fn test_register_read_write() {
        let mut agc = Agc::new();
        agc.set_reg_a(0x1234);
        assert_eq!(agc.reg_a(), 0x1234);

        agc.set_reg_l(0x5678);
        assert_eq!(agc.reg_l(), 0x5678);
    }

    #[test]
    fn test_zero_register() {
        let agc = Agc::new();
        assert_eq!(agc.read_mem(0o07), POS_ZERO);
    }

    #[test]
    fn test_editing_cyr() {
        let mut agc = Agc::new();
        // Write 0b10101010101010 to CYR
        agc.memory.erasable[reg::CYR] = 0b10101010101010;
        let result = agc.editing_cyr();
        // Cycle right: bit 1 (0) wraps to bit 15
        assert_eq!(result, 0b01010101010101);
    }

    #[test]
    fn test_editing_edop() {
        let mut agc = Agc::new();
        // EDOP shifts right 7 and masks upper bits
        agc.memory.erasable[reg::EDOP] = 0b011111110000000;
        let result = agc.editing_edop();
        assert_eq!(result, 0b1111111);
    }

    #[test]
    fn test_simple_program() {
        let mut agc = Agc::new();

        // Simple program in fixed-fixed (bank 2, address 04000):
        // CA 07     → Load ZERO register (0) into A
        // AD 07     → Add ZERO to A (A stays 0)
        let program: Vec<u16> = vec![
            0o30007, // CA 07 (load from ZERO register)
            0o60007, // AD 07 (add ZERO)
        ];

        agc.load_program(2, &program);
        agc.set_pc(0o4000);
        agc.start();

        // Execute first instruction (CA 07)
        agc.step();
        assert_eq!(agc.reg_a(), POS_ZERO);

        // Execute second instruction (AD 07)
        agc.step();
        assert_eq!(agc.reg_a(), POS_ZERO);
    }

    #[test]
    fn test_tc_and_return() {
        let mut agc = Agc::new();

        // Program:
        // 4000: TC 4002     → Jump to 4002, save return (4001) in Q
        // 4001: (should be skipped, then returned to)
        // 4002: CA 07       → Load 0 into A
        let program: Vec<u16> = vec![
            0o00000 | 0o4002, // TC 4002
            0o30007,          // CA 07 (return target)
            0o30007,          // CA 07 (subroutine body)
        ];

        agc.load_program(2, &program);
        agc.set_pc(0o4000);
        agc.start();

        // Execute TC 4002
        agc.step();
        assert_eq!(agc.reg_z(), 0o4002); // PC now at 4002
        assert_eq!(agc.reg_q(), 0o4001); // Return addr saved
    }

    #[test]
    fn test_snapshot() {
        let agc = Agc::new();
        let snap = agc.snapshot();
        assert_eq!(snap.a, POS_ZERO);
        assert!(!snap.running);
    }
}
