//! # AGC Core — Apollo Guidance Computer Simulator
//!
//! A faithful simulation of the Apollo Guidance Computer 4 (Block II) hardware
//! as documented in the apollo11-ai-walkthrough repository.
//!
//! ## Architecture Overview
//! - 15-bit word length + 1 parity bit (parity not simulated)
//! - 1's-complement arithmetic (two representations of zero: +0 and -0)
//! - 2,048 words erasable (RAM) in 8 banks
//! - 36,864 words fixed (ROM) in 36 banks
//! - 11 interrupt vectors
//! - I/O via memory-mapped channels

pub mod memory;
pub mod cpu;
pub mod instructions;
pub mod io;
pub mod interrupts;
pub mod timers;

pub use cpu::Agc;
