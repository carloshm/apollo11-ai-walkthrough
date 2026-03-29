# 🚀 Apollo Guidance Computer Simulator

A hardware-level simulation of the **Apollo Guidance Computer 4 (Block II)** — the computer that guided the Apollo 11 Lunar Module to the Moon's surface. Built in Rust, compiled to WebAssembly, and wrapped in a web interface with an interactive DSKY (Display & Keyboard) panel.

Based on the hardware architecture documented in the [apollo11-ai-walkthrough](../README.md) project.

## Architecture

```
simulator/
├── agc-core/          # Rust: AGC hardware simulator library
│   └── src/
│       ├── lib.rs           # Library root
│       ├── cpu.rs           # CPU state machine & register file
│       ├── memory.rs        # Memory subsystem (erasable + fixed, bank switching)
│       ├── instructions.rs  # Full instruction set (basic + extracodes)
│       ├── io.rs            # I/O channel subsystem (512 channels)
│       ├── interrupts.rs    # Interrupt controller (11 vectors)
│       └── timers.rs        # Timer/counter hardware (TIME1–6, CDU, PIPA)
├── agc-wasm/          # Rust → WASM: wasm-bindgen bindings
│   └── src/
│       └── lib.rs           # JavaScript-facing API
└── web/               # Web interface
    ├── index.html           # Main page
    ├── pkg/                 # WASM build output (generated)
    └── src/
        ├── main.js          # App entry point & run loop
        ├── dsky.js          # DSKY display/keyboard interface
        ├── debugger.js      # CPU state viewer & memory inspector
        ├── demos.js         # Built-in demo programs
        └── styles.css       # UI styling
```

## Hardware Simulated

The simulator faithfully reproduces the AGC4 Block II hardware as documented in the walkthrough:

| Component | Specification |
|-----------|---------------|
| **Word Length** | 15 data bits (1's complement, NOT 2's complement) |
| **Clock** | 1.024 MHz internal (~11.72 µs per machine cycle) |
| **Erasable (RAM)** | 2,048 words in 8 banks (E0–E7) |
| **Fixed (ROM)** | 36,864 words in 36 banks (core rope memory) |
| **Registers** | A, L, Q, EB, FB, Z (PC), BB, ZERO + editing registers |
| **Instruction Set** | 14 basic + 17 extracode instructions |
| **Interrupts** | 11 vectors (T3–T6RUPT, KEYRUPT, UPRUPT, DOWNRUPT, etc.) |
| **I/O Channels** | 512 channels for engine control, IMU, DSKY, radar |
| **Timers** | TIME1–6 with 10 ms and 1/1600 s resolution |

### Key Features

- **1's complement arithmetic** with proper +0/-0 handling
- **Bank switching** for both erasable and fixed memory
- **4-way CCS (Count, Compare, Skip)** — the AGC's unique conditional
- **INDEX self-modifying code** — array indexing via instruction modification
- **TS overflow-skip** — overflow detection built into the store instruction
- **Editing registers** (CYR, SR, CYL, EDOP) — hardware bit transformations on read
- **Interrupt system** with priority ordering, INHINT/RELINT, and RESUME

## Building

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- A web server for serving the web interface (e.g., `python3 -m http.server`)

### Build Steps

```bash
# 1. Build and test the Rust library
cd simulator
cargo test

# 2. Build the WASM package
cd agc-wasm
wasm-pack build --target web --out-dir ../web/pkg

# 3. Serve the web interface
cd ../web
python3 -m http.server 8080
# Open http://localhost:8080
```

## Web Interface

The web interface provides:

### DSKY Panel
An interactive replica of the astronaut's Display & Keyboard unit:
- **Status lights** (COMP ACTY, STBY, RESTART, etc.)
- **7-segment displays** showing PROG, VERB, NOUN, and three data registers (R1–R3)
- **19-key keyboard** with Verb/Noun command input
- **Keyboard shortcuts**: 0–9, V (Verb), N (Noun), Enter, Escape (CLR), +, -

### CPU Debugger
- **Register display** with change highlighting (A, L, Q, Z, EB, FB, BB)
- **CPU flags**: Overflow, Extend, Interrupts, ISR, Running, Engine
- **Execution stats**: Instruction count, elapsed simulation time, speed
- **Controls**: Step, Run, Stop, Reset, GOJAM

### Memory Inspector
- Browse erasable and fixed memory in octal format
- Program counter highlighting
- Auto-refresh during execution
- Navigate to any address

### Program Loader
Built-in demo programs:
- **Counter** — Simple increment loop
- **Add** — Sum two numbers (25 + 37 = 62)
- **Fibonacci** — Sequence generator
- **Countdown** — CCS-based count down to zero

Custom program input in octal machine code.

## Instruction Set Reference

### Basic Instructions (no EXTEND prefix)

| Mnemonic | Octal | Description |
|----------|-------|-------------|
| TC K | 0KKKK | Transfer control (subroutine call); Q ← return addr |
| CCS K | 1KKKK | Count, Compare, Skip (4-way branch) |
| DAS K | 20KKK | Double Add to Storage |
| LXCH K | 22KKK | Exchange L with memory |
| INCR K | 24KKK | Increment memory |
| ADS K | 26KKK | Add to Storage |
| CA K | 3KKKK | Clear and Add (load) |
| CS K | 4KKKK | Clear and Subtract (negate and load) |
| INDEX K | 50KKK | Modify next instruction |
| DXCH K | 52KKK | Double Exchange |
| TS K | 54KKK | Transfer to Storage (with overflow skip) |
| XCH K | 56KKK | Exchange A with memory |
| AD K | 6KKKK | Add |
| MASK K | 7KKKK | Bitwise AND |

### Extracodes (preceded by EXTEND)

| Mnemonic | Description |
|----------|-------------|
| DCA K | Double Clear and Add |
| DCS K | Double Clear and Subtract |
| MP K | Multiply |
| DV K | Divide |
| SU K | Subtract |
| BZF K | Branch if Zero to Fixed |
| BZMF K | Branch if Zero or Minus to Fixed |
| MSU K | Modular Subtract (2's complement for CDU) |
| AUG K | Augment (increment magnitude) |
| DIM K | Diminish (decrement magnitude) |
| READ K | Read I/O channel |
| WRITE K | Write I/O channel |
| RAND K | Read AND channel |
| WAND K | Write AND channel |

### Derived Instructions

| Mnemonic | Equivalent | Description |
|----------|-----------|-------------|
| RETURN | TC Q | Return from subroutine |
| INHINT | INDEX 17 | Disable interrupts |
| RELINT | INDEX 16 | Enable interrupts |
| RESUME | INDEX 25 | Return from ISR |
| NOOP | CA A | No operation |
| COM | CS A | Complement accumulator |
| DOUBLE | AD A | Double accumulator |
| EXTEND | INDEX 5777 | Next instruction is extracode |

## References

- [Apollo 11 source code](https://github.com/chrislgarry/Apollo-11) — Original AGC assembly
- [Virtual AGC](https://www.ibiblio.org/apollo/) — Comprehensive AGC documentation
- [AGC4 Assembly Language Manual](https://www.ibiblio.org/apollo/assembly_language_manual.html) — Instruction reference
- [apollo11-ai-walkthrough](../README.md) — AI-assisted code analysis this simulator is based on
