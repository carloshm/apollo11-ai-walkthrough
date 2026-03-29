//! I/O channel subsystem for the AGC.
//!
//! The AGC communicates with external hardware through 512 I/O channels.
//! Notable channels:
//! - Channel 1–6: L, Q, EB, FB, Z, BB registers (memory-mapped)
//! - Channel 7: Superbank bit
//! - Channel 10–14: Output channels (engine control, display, etc.)
//! - Channel 15–16: DSKY keyboard input
//! - Channel 30–33: Status/mode inputs
//!
//! Instructions READ, WRITE, RAND, WAND, ROR, WOR, RXOR access channels.

/// Number of I/O channels
pub const NUM_CHANNELS: usize = 512;

/// I/O Channel subsystem
#[derive(Clone)]
pub struct IoChannels {
    /// Channel data storage
    pub channels: [u16; NUM_CHANNELS],
}

/// Channel numbers for commonly used channels
pub mod channel {
    pub const L: usize = 0o001;
    pub const Q: usize = 0o002;
    pub const HISCALAR: usize = 0o003;
    pub const LOSCALAR: usize = 0o004;
    pub const PYJETS: usize = 0o005;
    pub const ROLLJETS: usize = 0o006;
    pub const SUPERBANK: usize = 0o007;
    pub const OUT0: usize = 0o010;
    pub const DSALMOUT: usize = 0o011;
    pub const CHAN12: usize = 0o012;
    pub const CHAN13: usize = 0o013;
    pub const CHAN14: usize = 0o014;
    pub const MNKEYIN: usize = 0o015;
    pub const NAVKEYIN: usize = 0o016;
    pub const CHAN30: usize = 0o030;
    pub const CHAN31: usize = 0o031;
    pub const CHAN32: usize = 0o032;
    pub const CHAN33: usize = 0o033;
    pub const DSKY_SIGN: usize = 0o163;
}

impl IoChannels {
    pub fn new() -> Self {
        let mut channels = [0u16; NUM_CHANNELS];
        // Channel 30–33 default values (input channels typically read as all-ones)
        channels[channel::CHAN30] = 0o37777;
        channels[channel::CHAN31] = 0o37777;
        channels[channel::CHAN32] = 0o37777;
        channels[channel::CHAN33] = 0o37777;
        Self { channels }
    }

    /// Read a channel value
    pub fn read(&self, channel: usize) -> u16 {
        if channel < NUM_CHANNELS {
            self.channels[channel] & 0x7FFF
        } else {
            0
        }
    }

    /// Write a channel value
    pub fn write(&mut self, channel: usize, value: u16) {
        if channel < NUM_CHANNELS {
            self.channels[channel] = value & 0x7FFF;
        }
    }

    /// Bitwise AND with channel (RAND/WAND)
    pub fn and_channel(&mut self, channel: usize, mask: u16) -> u16 {
        if channel < NUM_CHANNELS {
            let result = self.channels[channel] & mask & 0x7FFF;
            self.channels[channel] = result;
            result
        } else {
            0
        }
    }

    /// Bitwise OR with channel (ROR/WOR)
    pub fn or_channel(&mut self, channel: usize, value: u16) -> u16 {
        if channel < NUM_CHANNELS {
            let result = (self.channels[channel] | value) & 0x7FFF;
            self.channels[channel] = result;
            result
        } else {
            0
        }
    }

    /// Bitwise XOR with channel (RXOR)
    pub fn xor_channel(&mut self, channel: usize, value: u16) -> u16 {
        if channel < NUM_CHANNELS {
            let result = (self.channels[channel] ^ value) & 0x7FFF;
            self.channels[channel] = result;
            result
        } else {
            0
        }
    }

    /// Check if engine is on (channel 14, bit 4)
    pub fn is_engine_on(&self) -> bool {
        (self.channels[channel::CHAN14] & 0o20) != 0
    }

    /// Get superbank bit
    pub fn superbank(&self) -> bool {
        (self.channels[channel::SUPERBANK] & 0o100) != 0
    }
}

impl Default for IoChannels {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_read_write() {
        let mut io = IoChannels::new();
        io.write(0o010, 0o12345);
        assert_eq!(io.read(0o010), 0o12345);
    }

    #[test]
    fn test_channel_and() {
        let mut io = IoChannels::new();
        io.write(0o010, 0o77777);
        let result = io.and_channel(0o010, 0o12345);
        assert_eq!(result, 0o12345);
    }

    #[test]
    fn test_channel_or() {
        let mut io = IoChannels::new();
        io.write(0o010, 0o10000);
        let result = io.or_channel(0o010, 0o01234);
        assert_eq!(result, 0o11234);
    }

    #[test]
    fn test_engine_on() {
        let mut io = IoChannels::new();
        assert!(!io.is_engine_on());
        io.write(channel::CHAN14, 0o20);
        assert!(io.is_engine_on());
    }
}
