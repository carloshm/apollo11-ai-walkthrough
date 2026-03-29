//! WebAssembly bindings for the AGC simulator.
//!
//! Exposes the AGC simulator to JavaScript via wasm-bindgen.

use agc_core::cpu::AgcSnapshot;
use agc_core::Agc;
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// WASM-exposed AGC simulator wrapper
#[wasm_bindgen]
pub struct AgcSimulator {
    agc: Agc,
}

/// Serializable state for JavaScript consumption
#[derive(Serialize)]
pub struct AgcState {
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

impl From<AgcSnapshot> for AgcState {
    fn from(s: AgcSnapshot) -> Self {
        Self {
            a: s.a,
            l: s.l,
            q: s.q,
            eb: s.eb,
            fb: s.fb,
            z: s.z,
            bb: s.bb,
            a_overflow: s.a_overflow,
            extend: s.extend,
            running: s.running,
            interrupts_enabled: s.interrupts_enabled,
            in_isr: s.in_isr,
            instruction_count: s.instruction_count,
            elapsed_ms: s.elapsed_ms,
        }
    }
}

/// Serializable memory dump
#[derive(Serialize)]
pub struct MemoryDump {
    pub start: u16,
    pub words: Vec<u16>,
}

#[wasm_bindgen]
impl AgcSimulator {
    /// Create a new AGC simulator instance.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { agc: Agc::new() }
    }

    /// Reset the AGC to initial state.
    pub fn reset(&mut self) {
        self.agc = Agc::new();
    }

    /// Load a program into fixed memory at the given bank.
    /// `bank` is 0–35, `data` is an array of 15-bit words.
    pub fn load_program(&mut self, bank: usize, data: &[u16]) {
        self.agc.load_program(bank, data);
    }

    /// Load raw binary data into fixed memory at a flat address offset.
    pub fn load_fixed_raw(&mut self, offset: usize, data: &[u16]) {
        self.agc.memory.load_fixed(offset, data);
    }

    /// Set the program counter.
    pub fn set_pc(&mut self, addr: u16) {
        self.agc.set_pc(addr);
    }

    /// Get the current program counter.
    pub fn pc(&self) -> u16 {
        self.agc.pc()
    }

    /// Start the CPU.
    pub fn start(&mut self) {
        self.agc.start();
    }

    /// Stop the CPU.
    pub fn stop(&mut self) {
        self.agc.stop();
    }

    /// Execute a single instruction.
    /// Returns the number of MCTs consumed.
    pub fn step(&mut self) -> u64 {
        self.agc.step()
    }

    /// Run for the given number of MCTs.
    /// Returns actual MCTs executed.
    pub fn run_mcts(&mut self, max_mcts: u64) -> u64 {
        self.agc.run_mcts(max_mcts)
    }

    /// Run for approximately the given number of milliseconds of simulated time.
    pub fn run_ms(&mut self, ms: f64) -> u64 {
        let mcts = (ms * 1_000_000.0 / agc_core::timers::MCT_NS as f64) as u64;
        self.agc.run_mcts(mcts)
    }

    /// Get the current CPU state as a JavaScript object.
    pub fn get_state(&self) -> JsValue {
        let state: AgcState = self.agc.snapshot().into();
        serde_wasm_bindgen::to_value(&state).unwrap_or(JsValue::NULL)
    }

    /// Read a word from memory at the given address.
    pub fn read_memory(&self, addr: u16) -> u16 {
        self.agc.read_mem(addr)
    }

    /// Write a word to erasable memory.
    pub fn write_memory(&mut self, addr: u16, value: u16) {
        self.agc.write_mem(addr, value);
    }

    /// Read a range of memory words starting at `start` for `count` words.
    pub fn read_memory_range(&self, start: u16, count: u16) -> JsValue {
        let words: Vec<u16> = (0..count)
            .map(|i| self.agc.read_mem(start + i))
            .collect();
        let dump = MemoryDump { start, words };
        serde_wasm_bindgen::to_value(&dump).unwrap_or(JsValue::NULL)
    }

    /// Get individual register values.
    pub fn reg_a(&self) -> u16 { self.agc.reg_a() }
    pub fn reg_l(&self) -> u16 { self.agc.reg_l() }
    pub fn reg_q(&self) -> u16 { self.agc.reg_q() }
    pub fn reg_z(&self) -> u16 { self.agc.reg_z() }
    pub fn reg_eb(&self) -> u16 { self.agc.reg_eb() }
    pub fn reg_fb(&self) -> u16 { self.agc.reg_fb() }
    pub fn reg_bb(&self) -> u16 { self.agc.reg_bb() }

    /// Check if the CPU is running.
    pub fn is_running(&self) -> bool { self.agc.running }

    /// Get the total instruction count.
    pub fn instruction_count(&self) -> u64 { self.agc.instruction_count }

    /// Get elapsed simulation time in milliseconds.
    pub fn elapsed_ms(&self) -> f64 { self.agc.timers.elapsed_ms() }

    /// Send a DSKY key press (key code 0–31).
    pub fn dsky_key_press(&mut self, key_code: u16) {
        self.agc.dsky_key_press(key_code);
    }

    /// Read an I/O channel.
    pub fn read_channel(&self, channel: usize) -> u16 {
        self.agc.io.read(channel)
    }

    /// Write to an I/O channel.
    pub fn write_channel(&mut self, channel: usize, value: u16) {
        self.agc.io.write(channel, value);
    }

    /// Check if the engine is on (channel 14, bit 4).
    pub fn is_engine_on(&self) -> bool {
        self.agc.io.is_engine_on()
    }

    /// Trigger a GOJAM (hardware reset).
    pub fn gojam(&mut self) {
        self.agc.gojam();
    }

    /// Write to erasable memory directly (for initialization).
    pub fn write_erasable(&mut self, addr: usize, value: u16) {
        self.agc.memory.write_erasable(addr, value);
    }

    /// Read from erasable memory directly.
    pub fn read_erasable(&self, addr: usize) -> u16 {
        self.agc.memory.read_erasable(addr)
    }

    /// Enable or disable TIME6.
    pub fn set_time6_enabled(&mut self, enabled: bool) {
        self.agc.timers.time6_enabled = enabled;
    }
}

impl Default for AgcSimulator {
    fn default() -> Self {
        Self::new()
    }
}
