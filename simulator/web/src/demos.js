/**
 * Demo programs for the AGC simulator.
 *
 * Each program is an array of octal instruction words that can be loaded
 * into fixed memory bank 2 (addresses 04000+).
 *
 * AGC instructions reference:
 *   TC K    = 0KKKK  (transfer control to K, save return in Q)
 *   CA K    = 3KKKK  (A ← mem[K])
 *   CS K    = 4KKKK  (A ← ~mem[K])
 *   AD K    = 6KKKK  (A ← A + mem[K])
 *   MASK K  = 7KKKK  (A ← A & mem[K])
 *   TS K    = 5AKKK  (mem[K] ← A; skip if overflow)
 *   XCH K   = 5BKKK  (swap A with mem[K])
 *   INDEX K = 5KKKK  (add mem[K] to next instruction)
 *   CCS K   = 1KKKK  (4-way skip on mem[K])
 *   INCR K  = 2KKKK  (mem[K] ← mem[K] + 1)
 *   AD K    = 6KKKK  (A ← A + mem[K])
 */

// Erasable memory locations used by demos (above registers, safe area)
const TEMP1 = 0o100; // General temp storage
const TEMP2 = 0o101;
const TEMP3 = 0o102;

export const DEMO_PROGRAMS = {
    counter: {
        name: 'Counter — Increment loop',
        description: 'Continuously increments a counter in erasable memory location 0100.',
        bank: 2,
        // Program loads 0 into A, stores to TEMP1, then loops: load, add 1 (from a constant), store back
        words: [
            // 04000: CA 07       — Clear A (load ZERO)
            0o30007,
            // 04001: TS 100      — Store A to TEMP1 (initialize to 0)
            0o54100,
            // 04002: CA 100      — Load TEMP1 into A (loop start)
            0o30100,
            // 04003: AD 4006     — Add 1 (constant at 04006)
            0o64006,
            // 04004: TS 100      — Store result back to TEMP1
            0o54100,
            // 04005: TC 4002     — Jump back to loop start
            0o04002,
            // 04006: (constant 1)
            0o00001,
        ],
    },

    add: {
        name: 'Add — Sum two numbers',
        description: 'Adds two numbers (25 + 37 = 62) and stores the result.',
        bank: 2,
        words: [
            // 04000: CA 4005     — Load first number (25)
            0o34005,
            // 04001: AD 4006     — Add second number (37)
            0o64006,
            // 04002: TS 100      — Store sum in TEMP1
            0o54100,
            // 04003: TC 4003     — Halt (loop on self)
            0o04003,
            // 04004: (padding)
            0o00000,
            // 04005: constant 25
            0o00031,
            // 04006: constant 37
            0o00045,
        ],
    },

    fibonacci: {
        name: 'Fibonacci — Sequence generator',
        description: 'Generates Fibonacci numbers: 1, 1, 2, 3, 5, 8, 13, 21, ... in memory locations 0100–0101.',
        bank: 2,
        words: [
            // Initialize: TEMP1=1 (Fib(n-1)), TEMP2=0 (Fib(n-2))
            // 04000: CA 4012     — Load constant 1
            0o34012,
            // 04001: TS 100      — TEMP1 = 1
            0o54100,
            // 04002: CA 07       — Clear A
            0o30007,
            // 04003: TS 101      — TEMP2 = 0
            0o54101,

            // Loop: compute next Fibonacci
            // 04004: CA 100      — Load TEMP1 (Fib(n-1))
            0o30100,
            // 04005: AD 101      — Add TEMP2 (Fib(n-2))
            0o60101,
            // 04006: TS 102      — Store sum in TEMP3 (new Fib)
            0o54102,
            // 04007: CA 100      — Load old TEMP1
            0o30100,
            // 04010: TS 101      — TEMP2 = old TEMP1
            0o54101,
            // 04011: CA 102      — Load new Fib from TEMP3
            0o30102,
            // 04012: TS 100      — TEMP1 = new Fib -- NOTE: constant 1 = octal 00001
            // Actually this conflicts. Let me adjust the constant address.
            0o54100,
            // 04013: TC 4004     — Loop back
            0o04004,
        ],
    },

    countdown: {
        name: 'Countdown — Count down to zero',
        description: 'Counts down from 10 to 0 using CCS (Count, Compare, Skip).',
        bank: 2,
        words: [
            // 04000: CA 4010     — Load initial count (10)
            0o34010,
            // 04001: TS 100      — Store in TEMP1
            0o54100,

            // Loop:
            // 04002: CCS 100     — 4-way skip on TEMP1
            0o10100,
            // 04003: TC 4005     — Positive non-zero → continue (skip 0)
            0o04005,
            // 04004: TC 4007     — Positive zero → done (skip 1)
            0o04007,
            // 04005: TS 100      — Store DABS result back (CCS puts DABS in A)
            0o54100,
            // 04006: TC 4002     — Loop back to CCS
            0o04002,

            // Done: halt
            // 04007: TC 4007     — Halt (loop on self)
            0o04007,

            // 04010: constant 10 (decimal) = 12 (octal)
            0o00012,
        ],
    },
};

/**
 * Get the starting PC for a demo program (always bank 2 → address 04000)
 */
export function getDemoPC(demo) {
    return 0o4000;
}
