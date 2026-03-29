//! Memory subsystem for the AGC.
//!
//! Memory map (12-bit addresses within the 15-bit word):
//! - 0000–0061: Central and special registers (unswitched erasable)
//! - 0062–1377: Unswitched erasable RAM
//! - 1400–1777: Switched erasable (bank selected by EB register)
//! - 2000–3777: Common fixed (bank selected by FB register + superbank)
//! - 4000–7777: Fixed-fixed ROM (always accessible)
//!
//! Banks:
//! - Erasable banks E0–E7, each 256 words
//! - Fixed banks 00–43 (36 banks total), each 1024 words

/// Total erasable memory: 8 banks × 256 words = 2048 words
pub const ERASABLE_SIZE: usize = 2048;

/// Total fixed memory: 36 banks × 1024 words = 36864 words
pub const FIXED_SIZE: usize = 36864;

/// Number of erasable banks
pub const NUM_ERASABLE_BANKS: usize = 8;

/// Number of fixed banks
pub const NUM_FIXED_BANKS: usize = 36;

/// Words per erasable bank
pub const ERASABLE_BANK_SIZE: usize = 256;

/// Words per fixed bank
pub const FIXED_BANK_SIZE: usize = 1024;

/// 15-bit word mask
pub const WORD_MASK: u16 = 0x7FFF;

/// Sign bit position (bit 15 in 1-indexed, bit 14 in 0-indexed)
pub const SIGN_BIT: u16 = 0x4000;

/// Positive zero in 1's complement
pub const POS_ZERO: u16 = 0x0000;

/// Negative zero in 1's complement (all 1s in 15 bits)
pub const NEG_ZERO: u16 = 0x7FFF;

/// Overflow bit (bit 16, used in accumulator)
pub const OVERFLOW_BIT: u16 = 0x8000;

/// AGC Memory subsystem
#[derive(Clone)]
pub struct Memory {
    /// Erasable memory (RAM): 8 banks × 256 words = 2048 words
    pub erasable: [u16; ERASABLE_SIZE],
    /// Fixed memory (ROM): 36 banks × 1024 words = 36864 words
    pub fixed: [u16; FIXED_SIZE],
}

impl Memory {
    pub fn new() -> Self {
        Self {
            erasable: [0u16; ERASABLE_SIZE],
            fixed: [0u16; FIXED_SIZE],
        }
    }

    /// Read a word from erasable memory given a flat address (0–2047).
    pub fn read_erasable(&self, addr: usize) -> u16 {
        if addr < ERASABLE_SIZE {
            self.erasable[addr] & WORD_MASK
        } else {
            0
        }
    }

    /// Write a word to erasable memory given a flat address (0–2047).
    pub fn write_erasable(&mut self, addr: usize, value: u16) {
        if addr < ERASABLE_SIZE {
            self.erasable[addr] = value & WORD_MASK;
        }
    }

    /// Read a word from fixed memory given a flat address (0–36863).
    pub fn read_fixed(&self, addr: usize) -> u16 {
        if addr < FIXED_SIZE {
            self.fixed[addr] & WORD_MASK
        } else {
            0
        }
    }

    /// Load a program into fixed memory starting at a given offset.
    pub fn load_fixed(&mut self, offset: usize, data: &[u16]) {
        for (i, &word) in data.iter().enumerate() {
            let addr = offset + i;
            if addr < FIXED_SIZE {
                self.fixed[addr] = word & WORD_MASK;
            }
        }
    }

    /// Resolve a 12-bit address to a flat memory reference and read.
    /// Uses bank registers to resolve switched addresses.
    pub fn read(&self, addr: u16, eb: u16, fb: u16, superbank: bool) -> u16 {
        let addr12 = (addr & 0x0FFF) as usize;
        match addr12 {
            // Unswitched erasable: 0000–01377 (octal) = 0–767
            0..=0o1377 => self.read_erasable(addr12),
            // Switched erasable: 01400–01777 (octal) = 768–1023
            0o1400..=0o1777 => {
                let bank = (eb as usize) & 0x07;
                let offset = addr12 - 0o1400;
                let flat = bank * ERASABLE_BANK_SIZE + offset;
                self.read_erasable(flat)
            }
            // Common fixed (bank-switched): 02000–03777 (octal) = 1024–2047
            0o2000..=0o3777 => {
                let bank = self.resolve_fixed_bank(fb, superbank);
                let offset = addr12 - 0o2000;
                let flat = bank * FIXED_BANK_SIZE + offset;
                self.read_fixed(flat)
            }
            // Fixed-fixed: 04000–07777 (octal) = 2048–4095
            0o4000..=0o7777 => {
                // Fixed-fixed maps to banks 02 and 03
                let offset = addr12 - 0o4000;
                let bank = if offset < FIXED_BANK_SIZE { 2 } else { 3 };
                let bank_offset = offset % FIXED_BANK_SIZE;
                let flat = bank * FIXED_BANK_SIZE + bank_offset;
                self.read_fixed(flat)
            }
            _ => 0,
        }
    }

    /// Resolve a 12-bit address and write (only erasable addresses are writable).
    pub fn write(&mut self, addr: u16, value: u16, eb: u16) {
        let addr12 = (addr & 0x0FFF) as usize;
        match addr12 {
            0..=0o1377 => {
                self.write_erasable(addr12, value);
            }
            0o1400..=0o1777 => {
                let bank = (eb as usize) & 0x07;
                let offset = addr12 - 0o1400;
                let flat = bank * ERASABLE_BANK_SIZE + offset;
                self.write_erasable(flat, value);
            }
            _ => {
                // Cannot write to fixed memory
            }
        }
    }

    /// Resolve FB register + superbank bit to a fixed bank number (0–35).
    fn resolve_fixed_bank(&self, fb: u16, superbank: bool) -> usize {
        // FB register contains the bank number in bits 14–10
        // (5-bit field: bits 14..10 of the 15-bit word)
        let bank5 = ((fb >> 10) & 0x1F) as usize;
        if superbank && bank5 >= 030 {
            // Superbank: banks 030–033 map to banks 040–043 (banks 32–35)
            bank5 + 8
        } else {
            bank5
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_erasable_read_write() {
        let mut mem = Memory::new();
        mem.write_erasable(100, 0x1234);
        assert_eq!(mem.read_erasable(100), 0x1234);
    }

    #[test]
    fn test_word_mask() {
        let mut mem = Memory::new();
        // Writing a value with bit 15 set should be masked
        mem.write_erasable(50, 0xFFFF);
        assert_eq!(mem.read_erasable(50), WORD_MASK);
    }

    #[test]
    fn test_unswitched_erasable_access() {
        let mut mem = Memory::new();
        mem.write_erasable(100, 0x2345);
        assert_eq!(mem.read(100, 0, 0, false), 0x2345);
    }

    #[test]
    fn test_switched_erasable_bank() {
        let mut mem = Memory::new();
        // Bank 3, offset 0 → flat address = 3 * 256 + 0 = 768
        mem.write_erasable(768, 0x5678);
        // Address 01400 with EB=3
        assert_eq!(mem.read(0o1400, 3, 0, false), 0x5678);
    }

    #[test]
    fn test_fixed_fixed_access() {
        let mut mem = Memory::new();
        // Fixed-fixed address 04000 maps to bank 2, offset 0
        let flat = 2 * FIXED_BANK_SIZE;
        mem.fixed[flat] = 0x1111;
        assert_eq!(mem.read(0o4000, 0, 0, false), 0x1111);
    }
}
