//! AGC instruction set implementation.
//!
//! The AGC uses a 3-bit opcode (bits 14–12) + 12-bit address field.
//! Some instructions use the "quarter code" (bits 11–10) for disambiguation.
//!
//! Basic instructions (no EXTEND prefix):
//!   TC, CCS, CA, CS, AD, MASK, INDEX, TS, XCH, DAS, DXCH, LXCH, INCR, ADS
//!
//! Extracodes (require EXTEND prefix = INDEX 5777):
//!   DCA, DCS, MP, DV, SU, BZF, BZMF, MSU, READ, WRITE, RAND, WAND, ROR, WOR, RXOR, AUG, DIM

use crate::cpu::Agc;
use crate::memory::{NEG_ZERO, POS_ZERO, SIGN_BIT, WORD_MASK};

/// Opcode extraction from a 15-bit instruction word
pub fn opcode(word: u16) -> u16 {
    (word >> 12) & 0x07
}

/// Quarter-code extraction (bits 11–10)
pub fn quarter_code(word: u16) -> u16 {
    (word >> 10) & 0x03
}

/// Address field extraction (12-bit)
pub fn address_field(word: u16) -> u16 {
    word & 0x0FFF
}

/// 1's complement addition of two 15-bit values.
/// Returns 16-bit result where bit 15 is the overflow/sign.
pub fn ones_complement_add(a: u16, b: u16) -> u16 {
    let a32 = sign_extend_15_to_32(a);
    let b32 = sign_extend_15_to_32(b);
    let sum = a32.wrapping_add(b32);

    // Handle end-around carry for 1's complement
    // Convert back to 16-bit with possible overflow indication
    overflow_correct(sum)
}

/// Sign-extend a 15-bit value to 32-bit signed
fn sign_extend_15_to_32(val: u16) -> i32 {
    let v = (val & WORD_MASK) as i32;
    if v & (SIGN_BIT as i32) != 0 {
        v | !((WORD_MASK) as i32) // sign extend
    } else {
        v
    }
}

/// Convert a 32-bit arithmetic result back to 16-bit AGC representation.
/// Bit 15 of result indicates overflow.
fn overflow_correct(val: i32) -> u16 {
    if val > 0x3FFF {
        // Positive overflow
        let corrected = (val as u16) & WORD_MASK;
        corrected | 0x8000 // set overflow bit
    } else if val < -(0x3FFF_i32) {
        // Negative overflow
        let corrected = (val as u16) & WORD_MASK;
        corrected | 0x8000
    } else if val < 0 {
        // Negative, no overflow: 1's complement
        (val as u16) & WORD_MASK
    } else {
        // Positive, no overflow
        (val as u16) & WORD_MASK
    }
}

/// 1's complement negate
pub fn ones_complement_negate(val: u16) -> u16 {
    (!val) & WORD_MASK
}

/// Check if a 16-bit value has overflow (bit 15 set differently from bit 14)
pub fn has_overflow(val: u16) -> bool {
    (val & 0x8000) != 0
}

/// Get the diminished absolute value (DABS) used by CCS:
/// If positive (non-zero): value - 1
/// If positive zero: 0
/// If negative (non-zero): |value| - 1
/// If negative zero: 0
pub fn diminished_abs(val: u16) -> u16 {
    let v = val & WORD_MASK;
    if v == POS_ZERO || v == NEG_ZERO {
        POS_ZERO
    } else if v & SIGN_BIT == 0 {
        // Positive, non-zero: subtract 1
        v - 1
    } else {
        // Negative, non-zero: take magnitude - 1
        let magnitude = ones_complement_negate(v);
        if magnitude == 0 {
            POS_ZERO
        } else {
            magnitude - 1
        }
    }
}

/// Execute a basic (non-extracode) instruction.
/// Returns the number of MCTs consumed (typically 1 or 2).
pub fn execute_basic(agc: &mut Agc, instruction: u16) -> u64 {
    let op = opcode(instruction);
    let qc = quarter_code(instruction);
    let addr = address_field(instruction);

    match op {
        0o0 => exec_tc(agc, addr),       // TC (Transfer Control)
        0o1 => {
            match qc {
                0o0 => exec_ccs(agc, addr),   // CCS
                0o1 => exec_das(agc, addr),   // DAS (addr is odd)
                _ => exec_ccs(agc, addr),
            }
        }
        0o2 => {
            match qc {
                0o0 => exec_lxch(agc, addr),  // LXCH
                0o1 => exec_incr(agc, addr),  // INCR
                0o2 => exec_ads(agc, addr),   // ADS
                _ => exec_ca(agc, address_field(instruction)),
            }
        }
        0o3 => exec_ca(agc, addr),       // CA (Clear and Add)
        0o4 => exec_cs(agc, addr),       // CS (Clear and Subtract)
        0o5 => {
            match qc {
                0o0 => exec_index(agc, addr), // INDEX
                0o1 => exec_dxch(agc, addr),  // DXCH (addr is odd)
                0o2 => exec_ts(agc, addr),    // TS
                0o3 => exec_xch(agc, addr),   // XCH
                _ => 1,
            }
        }
        0o6 => exec_ad(agc, addr),       // AD (Add)
        0o7 => exec_mask(agc, addr),     // MASK
        _ => 1, // Unknown opcode
    }
}

/// Execute an extracode instruction (preceded by EXTEND).
/// Returns the number of MCTs consumed.
pub fn execute_extracode(agc: &mut Agc, instruction: u16) -> u64 {
    let op = opcode(instruction);
    let qc = quarter_code(instruction);
    let addr = address_field(instruction);

    match op {
        0o0 => exec_read(agc, addr),
        0o1 => {
            match qc {
                0o0 => exec_dv(agc, addr),
                0o1 => exec_bzf(agc, addr),
                _ => exec_dv(agc, addr),
            }
        }
        0o2 => {
            match qc {
                0o0 => exec_msu(agc, addr),
                0o1 => exec_dim(agc, addr),
                0o2 => exec_aug(agc, addr),
                _ => 1,
            }
        }
        0o3 => exec_dca(agc, addr),      // DCA
        0o4 => exec_dcs(agc, addr),      // DCS
        0o5 => {
            match qc {
                0o0 => exec_index(agc, addr), // INDEX (extracode form)
                0o1 => {
                    // SU or BZMF depending on address range
                    if addr >= 0o2000 {
                        exec_bzmf(agc, addr)
                    } else {
                        exec_su(agc, addr)
                    }
                }
                0o2 => exec_mp(agc, addr),    // MP
                0o3 => exec_mp(agc, addr),
                _ => 1,
            }
        }
        0o6 => exec_su(agc, addr),
        0o7 => {
            match qc {
                0o0 => exec_read(agc, addr),  // READ
                0o1 => exec_write(agc, addr), // WRITE
                0o2 => exec_rand(agc, addr),  // RAND
                0o3 => exec_wand(agc, addr),  // WAND
                _ => 1,
            }
        }
        _ => 1,
    }
}

// ===== Basic Instruction Implementations =====

/// TC (Transfer Control): Jump to K, save return in Q
fn exec_tc(agc: &mut Agc, addr: u16) -> u64 {
    let target = addr;
    agc.write_reg_q(agc.reg_z());
    agc.set_reg_z(target);
    1
}

/// CCS (Count, Compare, and Skip): 4-way branch based on value at K
fn exec_ccs(agc: &mut Agc, addr: u16) -> u64 {
    let val = agc.read_mem(addr);
    let dabs = diminished_abs(val);
    agc.set_reg_a(dabs);

    let v = val & WORD_MASK;
    let skip = if v != POS_ZERO && (v & SIGN_BIT) == 0 {
        0 // Positive non-zero: skip 0 (execute next)
    } else if v == POS_ZERO {
        1 // Positive zero: skip 1
    } else if v != NEG_ZERO && (v & SIGN_BIT) != 0 {
        2 // Negative non-zero: skip 2
    } else {
        3 // Negative zero: skip 3
    };

    let z = agc.reg_z();
    agc.set_reg_z(z + skip);
    2
}

/// CA (Clear and Add): A ← mem[K]
fn exec_ca(agc: &mut Agc, addr: u16) -> u64 {
    let val = agc.read_mem(addr);
    agc.set_reg_a(val);
    2
}

/// CS (Clear and Subtract): A ← ~mem[K] (1's complement negate)
fn exec_cs(agc: &mut Agc, addr: u16) -> u64 {
    let val = agc.read_mem(addr);
    agc.set_reg_a(ones_complement_negate(val));
    2
}

/// AD (Add): A ← A + mem[K]
fn exec_ad(agc: &mut Agc, addr: u16) -> u64 {
    let val = agc.read_mem(addr);
    let a = agc.reg_a();
    let result = ones_complement_add(a, val);
    agc.set_reg_a_with_overflow(result);
    2
}

/// MASK (Bitwise AND): A ← A & mem[K]
fn exec_mask(agc: &mut Agc, addr: u16) -> u64 {
    let val = agc.read_mem(addr);
    let a = agc.reg_a();
    agc.set_reg_a((a & val) & WORD_MASK);
    2
}

/// INDEX: Add mem[K] to next instruction before executing it
fn exec_index(agc: &mut Agc, addr: u16) -> u64 {
    // Special cases: INDEX 17 = RESUME, INDEX 16 = RELINT, INDEX 25 = RESUME
    match addr {
        0o17 => {
            // INHINT — disable interrupts
            agc.interrupts.inhibit();
            1
        }
        0o16 => {
            // RELINT — enable interrupts
            agc.interrupts.release();
            1
        }
        0o25 => {
            // RESUME — return from interrupt
            let (a, l, q, bb, z) = agc.interrupts.exit_isr();
            agc.set_reg_a(a);
            agc.set_reg_l(l);
            agc.write_reg_q(q);
            agc.set_reg_bb(bb);
            agc.set_reg_z(z);
            1
        }
        _ => {
            let index_val = agc.read_mem(addr);
            agc.index_value = Some(index_val);
            1
        }
    }
}

/// TS (Transfer to Storage): mem[K] ← A; skip next if overflow
fn exec_ts(agc: &mut Agc, addr: u16) -> u64 {
    let a = agc.reg_a_with_overflow();
    if has_overflow(a) {
        // Overflow: store sign-corrected value, set A to ±1, skip next
        let sign = if a & SIGN_BIT != 0 { SIGN_BIT | 1 } else { 0 };
        let corrected = a & WORD_MASK;
        agc.write_mem(addr, corrected);
        // A = +1 or -1 depending on overflow direction
        if a & 0x8000 != 0 && a & SIGN_BIT != 0 {
            agc.set_reg_a(NEG_ZERO - 1); // -1 in 1's complement
        } else {
            let _ = sign;
            agc.set_reg_a(0x0001); // +1
        }
        let z = agc.reg_z();
        agc.set_reg_z(z + 1); // Skip next instruction
    } else {
        agc.write_mem(addr, a & WORD_MASK);
    }
    2
}

/// XCH (Exchange): Swap A with mem[K]
fn exec_xch(agc: &mut Agc, addr: u16) -> u64 {
    let mem_val = agc.read_mem(addr);
    let a_val = agc.reg_a();
    agc.write_mem(addr, a_val);
    agc.set_reg_a(mem_val);
    2
}

/// LXCH (L Exchange): Swap L with mem[K]
fn exec_lxch(agc: &mut Agc, addr: u16) -> u64 {
    let mem_val = agc.read_mem(addr);
    let l_val = agc.reg_l();
    agc.write_mem(addr, l_val);
    agc.set_reg_l(mem_val);
    2
}

/// INCR: mem[K] ← mem[K] + 1
fn exec_incr(agc: &mut Agc, addr: u16) -> u64 {
    let val = agc.read_mem(addr);
    let result = ones_complement_add(val, 1);
    agc.write_mem(addr, result & WORD_MASK);
    2
}

/// ADS (Add to Storage): mem[K] ← A + mem[K]; A ← result
fn exec_ads(agc: &mut Agc, addr: u16) -> u64 {
    let val = agc.read_mem(addr);
    let a = agc.reg_a();
    let result = ones_complement_add(a, val);
    agc.write_mem(addr, result & WORD_MASK);
    agc.set_reg_a_with_overflow(result);
    2
}

/// DAS (Double Add to Storage): DP add
fn exec_das(agc: &mut Agc, addr: u16) -> u64 {
    let k = addr & 0xFFFE; // Ensure even address
    let a = agc.reg_a();
    let l = agc.reg_l();
    let mem_hi = agc.read_mem(k);
    let mem_lo = agc.read_mem(k + 1);

    // Add low words
    let lo_sum = ones_complement_add(l, mem_lo);
    let carry = if has_overflow(lo_sum) { 1u16 } else { 0u16 };
    agc.write_mem(k + 1, lo_sum & WORD_MASK);

    // Add high words + carry
    let hi_sum = ones_complement_add(a, mem_hi);
    let hi_sum = ones_complement_add(hi_sum & WORD_MASK, carry);
    agc.write_mem(k, hi_sum & WORD_MASK);

    agc.set_reg_a(POS_ZERO);
    agc.set_reg_l(POS_ZERO);
    3
}

/// DXCH (Double Exchange): Swap A,L with mem[K],mem[K+1]
fn exec_dxch(agc: &mut Agc, addr: u16) -> u64 {
    let k = addr & 0xFFFE;
    let a = agc.reg_a();
    let l = agc.reg_l();
    let mem_hi = agc.read_mem(k);
    let mem_lo = agc.read_mem(k + 1);
    agc.write_mem(k, a);
    agc.write_mem(k + 1, l);
    agc.set_reg_a(mem_hi);
    agc.set_reg_l(mem_lo);
    3
}

// ===== Extracode Instruction Implementations =====

/// DCA (Double Clear and Add): Load DP from K,K+1 into A,L
fn exec_dca(agc: &mut Agc, addr: u16) -> u64 {
    let k = addr & 0xFFFE;
    let hi = agc.read_mem(k);
    let lo = agc.read_mem(k + 1);
    agc.set_reg_a(hi);
    agc.set_reg_l(lo);
    3
}

/// DCS (Double Clear and Subtract): Load negated DP
fn exec_dcs(agc: &mut Agc, addr: u16) -> u64 {
    let k = addr & 0xFFFE;
    let hi = agc.read_mem(k);
    let lo = agc.read_mem(k + 1);
    agc.set_reg_a(ones_complement_negate(hi));
    agc.set_reg_l(ones_complement_negate(lo));
    3
}

/// MP (Multiply): A × mem[K] → A (high), L (low)
fn exec_mp(agc: &mut Agc, addr: u16) -> u64 {
    let a_val = agc.reg_a();
    let k_val = agc.read_mem(addr);

    // Convert to signed for multiplication
    let a_sign = a_val & SIGN_BIT != 0;
    let k_sign = k_val & SIGN_BIT != 0;
    let a_mag = if a_sign { ones_complement_negate(a_val) } else { a_val } as u32;
    let k_mag = if k_sign { ones_complement_negate(k_val) } else { k_val } as u32;

    let product = a_mag * k_mag;
    let result_sign = a_sign != k_sign;

    // Split into high (bits 28–14) and low (bits 13–0)
    let hi = ((product >> 14) & 0x3FFF) as u16;
    let lo = (product & 0x3FFF) as u16;

    let hi = if result_sign && hi != 0 { ones_complement_negate(hi) } else { hi };
    let lo = if result_sign && lo != 0 { ones_complement_negate(lo) } else { lo };

    agc.set_reg_a(hi & WORD_MASK);
    agc.set_reg_l(lo & WORD_MASK);
    3
}

/// DV (Divide): DP(A,L) ÷ SP(K) → quotient in A, remainder in L
fn exec_dv(agc: &mut Agc, addr: u16) -> u64 {
    let k_val = agc.read_mem(addr);
    let a_val = agc.reg_a();
    let l_val = agc.reg_l();

    if k_val == POS_ZERO || k_val == NEG_ZERO {
        // Division by zero: set overflow
        agc.set_reg_a(if a_val & SIGN_BIT != 0 { NEG_ZERO } else { 0x3FFF });
        agc.set_reg_l(POS_ZERO);
        return 6;
    }

    let a_sign = a_val & SIGN_BIT != 0;
    let k_sign = k_val & SIGN_BIT != 0;
    let a_mag = if a_sign { ones_complement_negate(a_val) } else { a_val } as u32;
    let l_mag = if a_sign { ones_complement_negate(l_val) } else { l_val } as u32;
    let k_mag = if k_sign { ones_complement_negate(k_val) } else { k_val } as u32;

    let dividend = (a_mag << 14) | l_mag;
    let quotient = if k_mag != 0 { dividend / k_mag } else { 0x3FFF };
    let remainder = if k_mag != 0 { dividend % k_mag } else { 0 };

    let q_sign = a_sign != k_sign;
    let q = (quotient & 0x3FFF) as u16;
    let r = (remainder & 0x3FFF) as u16;

    let q = if q_sign && q != 0 { ones_complement_negate(q) } else { q };
    let r = if a_sign && r != 0 { ones_complement_negate(r) } else { r };

    agc.set_reg_a(q & WORD_MASK);
    agc.set_reg_l(r & WORD_MASK);
    6
}

/// SU (Subtract): A ← A - mem[K]
fn exec_su(agc: &mut Agc, addr: u16) -> u64 {
    let val = agc.read_mem(addr);
    let a = agc.reg_a();
    let neg_val = ones_complement_negate(val);
    let result = ones_complement_add(a, neg_val);
    agc.set_reg_a_with_overflow(result);
    2
}

/// BZF (Branch Zero to Fixed): Jump if A = ±0
fn exec_bzf(agc: &mut Agc, addr: u16) -> u64 {
    let a = agc.reg_a();
    if a == POS_ZERO || a == NEG_ZERO {
        agc.set_reg_z(addr);
    }
    1
}

/// BZMF (Branch Zero or Minus to Fixed): Jump if A ≤ 0
fn exec_bzmf(agc: &mut Agc, addr: u16) -> u64 {
    let a = agc.reg_a();
    if a == POS_ZERO || a == NEG_ZERO || (a & SIGN_BIT) != 0 {
        agc.set_reg_z(addr);
    }
    1
}

/// MSU (Modular Subtract): 2's complement subtract for CDU angles
fn exec_msu(agc: &mut Agc, addr: u16) -> u64 {
    let val = agc.read_mem(addr);
    let a = agc.reg_a();
    // MSU does 2's complement subtraction, then converts back to 1's complement
    let a_2c = if a & SIGN_BIT != 0 { !a & WORD_MASK } else { a };
    let v_2c = if val & SIGN_BIT != 0 { !val & WORD_MASK } else { val };
    let diff = a_2c.wrapping_sub(v_2c) & WORD_MASK;
    agc.set_reg_a(diff);
    2
}

/// AUG (Augment): Increment magnitude of mem[K]
fn exec_aug(agc: &mut Agc, addr: u16) -> u64 {
    let val = agc.read_mem(addr);
    let result = if val & SIGN_BIT != 0 {
        // Negative: subtract 1 from 1's complement (increase magnitude)
        if val == NEG_ZERO { NEG_ZERO - 1 } else { val - 1 }
    } else {
        // Positive: add 1
        val + 1
    };
    agc.write_mem(addr, result & WORD_MASK);
    2
}

/// DIM (Diminish): Decrement magnitude of mem[K] toward zero
fn exec_dim(agc: &mut Agc, addr: u16) -> u64 {
    let val = agc.read_mem(addr);
    let result = if val == POS_ZERO || val == NEG_ZERO {
        val // Already zero
    } else if val & SIGN_BIT != 0 {
        // Negative: add 1 to 1's complement (decrease magnitude)
        (val + 1) & WORD_MASK
    } else {
        // Positive: subtract 1
        val - 1
    };
    agc.write_mem(addr, result);
    2
}

/// READ: A ← channel[K]
fn exec_read(agc: &mut Agc, addr: u16) -> u64 {
    let channel = addr as usize;
    let val = agc.io.read(channel);
    agc.set_reg_a(val);
    2
}

/// WRITE: channel[K] ← A
fn exec_write(agc: &mut Agc, addr: u16) -> u64 {
    let channel = addr as usize;
    let a = agc.reg_a();
    agc.io.write(channel, a);
    2
}

/// RAND (Read AND): A ← A & channel[K]
fn exec_rand(agc: &mut Agc, addr: u16) -> u64 {
    let channel = addr as usize;
    let a = agc.reg_a();
    let val = agc.io.read(channel);
    agc.set_reg_a((a & val) & WORD_MASK);
    2
}

/// WAND (Write AND): channel[K] ← channel[K] & A
fn exec_wand(agc: &mut Agc, addr: u16) -> u64 {
    let channel = addr as usize;
    let a = agc.reg_a();
    let result = agc.io.and_channel(channel, a);
    agc.set_reg_a(result);
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ones_complement_add_positive() {
        let result = ones_complement_add(5, 3);
        assert_eq!(result & WORD_MASK, 8);
    }

    #[test]
    fn test_ones_complement_negate() {
        let val = 0x1234;
        let neg = ones_complement_negate(val);
        // Adding a value and its negation should give ±0
        let sum = ones_complement_add(val, neg) & WORD_MASK;
        assert!(sum == POS_ZERO || sum == NEG_ZERO);
    }

    #[test]
    fn test_diminished_abs_positive() {
        assert_eq!(diminished_abs(5), 4);
    }

    #[test]
    fn test_diminished_abs_zero() {
        assert_eq!(diminished_abs(POS_ZERO), POS_ZERO);
        assert_eq!(diminished_abs(NEG_ZERO), POS_ZERO);
    }

    #[test]
    fn test_diminished_abs_negative() {
        let neg_5 = ones_complement_negate(5);
        assert_eq!(diminished_abs(neg_5), 4);
    }
}
