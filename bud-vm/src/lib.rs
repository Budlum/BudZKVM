use bud_isa::{Instruction, Opcode};
use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VmError {
    OutOfGas,
    AssertionFailed,
    StackUnderflow,
    StackOverflow,
    InvalidOpcode(String),
    InvalidPc,
    InvalidMemoryAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub success: bool,
    pub error: Option<VmError>,
    pub gas_used: u64,
    pub exit_code: u64,
    pub events: Vec<u64>,
    pub final_pc: u64,
    pub trace_len: u64,
    pub state_writes_digest: [u8; 32],
}

pub struct Vm {
    pub registers: [u64; 32],
    pub pc: usize,
    pub stack: Vec<u64>,
    pub memory: Vec<u8>,
    pub storage: std::collections::HashMap<i32, u64>,
    pub events: Vec<u64>,
    pub context: Context,
    pub trace: Vec<Step>,
    pub halted: bool,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub error: Option<VmError>,
    pub state_writes: Vec<(i32, u64)>,
}

pub struct Context {
    pub sender: u64,
    pub nonce: u64,
    pub block_height: u64,
}

#[derive(Debug, Clone)]
pub struct Step {
    pub pc: usize,
    pub next_pc: usize,
    pub instruction: Instruction,
    pub src1_idx: u8,
    pub src2_idx: u8,
    pub dst_idx: u8,
    pub src1_val: u64,
    pub src2_val: u64,
    pub dst_val: u64,
    pub registers: [u64; 32],
    pub memory_addr: Option<usize>,
    pub memory_val: Option<u64>,
    pub is_memory_write: bool,
    pub stack_pointer: usize,
}

pub fn field_inverse_goldilocks(val: u64) -> u64 {
    const P: u64 = 18446744069414584321;
    if val == 0 {
        return 0;
    }
    let mut exp = P - 2;
    let mut base = val as u128;
    let mut res = 1u128;
    while exp > 0 {
        if exp & 1 == 1 {
            res = (res * base) % P as u128;
        }
        base = (base * base) % P as u128;
        exp >>= 1;
    }
    res as u64
}

impl Vm {
    pub fn new(memory_size: usize) -> Self {
        Self::with_gas_limit(memory_size, 1_000_000)
    }

    pub fn with_gas_limit(memory_size: usize, gas_limit: u64) -> Self {
        Self {
            registers: [0; 32],
            pc: 0,
            stack: Vec::new(),
            memory: vec![0; memory_size],
            storage: std::collections::HashMap::new(),
            events: Vec::new(),
            context: Context {
                sender: 0,
                nonce: 0,
                block_height: 0,
            },
            trace: Vec::new(),
            halted: false,
            gas_used: 0,
            gas_limit,
            error: None,
            state_writes: Vec::new(),
        }
    }

    pub fn consume_gas(&mut self, amount: u64) -> Result<(), VmError> {
        self.gas_used = self.gas_used.saturating_add(amount);
        if self.gas_used > self.gas_limit {
            self.halted = true;
            self.error = Some(VmError::OutOfGas);
            return Err(VmError::OutOfGas);
        }
        Ok(())
    }

    pub fn step(&mut self, program: &[u64]) -> Result<(), VmError> {
        self.registers[0] = 0; // Enforce r0 is always 0
        if self.halted {
            return Ok(());
        }
        if self.pc >= program.len() {
            self.halted = true;
            self.error = Some(VmError::InvalidPc);
            return Err(VmError::InvalidPc);
        }

        let raw_inst = program[self.pc];
        let inst = match Instruction::decode(raw_inst) {
            Ok(i) => i,
            Err(e) => {
                self.halted = true;
                self.error = Some(VmError::InvalidOpcode(e.clone()));
                return Err(VmError::InvalidOpcode(e));
            }
        };

        let cur_pc = self.pc;
        self.consume_gas(Self::gas_cost(inst.opcode))?;

        let src1_idx = inst.rs1;
        let src2_idx = inst.rs2;
        let dst_idx = inst.rd;
        let src1_val = self.registers[src1_idx as usize];
        let src2_val = self.registers[src2_idx as usize];

        let mut memory_addr = None;
        let mut memory_val = None;
        let mut is_memory_write = false;

        let (dst_val, next_pc) = match inst.opcode {
            Opcode::Halt => {
                self.halted = true;
                (0, cur_pc)
            }
            Opcode::Add => {
                let result = src1_val.wrapping_add(src2_val);
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Sub => {
                let result = src1_val.wrapping_sub(src2_val);
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Mul => {
                let result = src1_val.wrapping_mul(src2_val);
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Div => {
                const P: u64 = 18446744069414584321;
                let result = if src2_val != 0 {
                    let inv = field_inverse_goldilocks(src2_val);
                    ((src1_val as u128 * inv as u128) % P as u128) as u64
                } else {
                    0
                };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Inv => {
                let result = if src1_val != 0 {
                    field_inverse_goldilocks(src1_val)
                } else {
                    0
                };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::And => {
                let result = src1_val & src2_val;
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Or => {
                let result = src1_val | src2_val;
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Xor => {
                let result = src1_val ^ src2_val;
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Not => {
                let result = if src1_val == 0 { 1 } else { 0 };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Load => {
                let result = if src1_idx == 0 {
                    inst.imm as u64
                } else if let Some(addr) =
                    Self::memory_word_addr(src1_val, inst.imm, self.memory.len())
                {
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(&self.memory[addr..addr + 8]);
                    memory_addr = Some(addr);
                    let val = u64::from_le_bytes(bytes);
                    memory_val = Some(val);
                    val
                } else {
                    self.halted = true;
                    self.error = Some(VmError::InvalidMemoryAccess);
                    return Err(VmError::InvalidMemoryAccess);
                };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Store => {
                if let Some(addr) = Self::memory_word_addr(src1_val, inst.imm, self.memory.len()) {
                    let bytes = src2_val.to_le_bytes();
                    self.memory[addr..addr + 8].copy_from_slice(&bytes);
                    memory_addr = Some(addr);
                    memory_val = Some(src2_val);
                    is_memory_write = true;
                } else {
                    self.halted = true;
                    self.error = Some(VmError::InvalidMemoryAccess);
                    return Err(VmError::InvalidMemoryAccess);
                }
                self.pc += 1;
                (0, cur_pc + 1)
            }
            Opcode::Jmp => {
                let target = (cur_pc as i64 + inst.imm as i64) as usize;
                self.pc = target;
                (0, target)
            }
            Opcode::Jnz => {
                let target = if src1_val != 0 {
                    (cur_pc as i64 + inst.imm as i64) as usize
                } else {
                    cur_pc + 1
                };
                self.pc = target;
                (0, target)
            }
            Opcode::Call => {
                if self.stack.len() >= 1024 {
                    self.halted = true;
                    self.error = Some(VmError::StackOverflow);
                    return Err(VmError::StackOverflow);
                }
                let target = (cur_pc as i64 + inst.imm as i64) as usize;
                self.stack.push((cur_pc + 1) as u64);
                self.pc = target;
                ((cur_pc + 1) as u64, target)
            }
            Opcode::Ret => {
                let target = match self.stack.pop() {
                    Some(val) => val as usize,
                    None => {
                        self.halted = true;
                        self.error = Some(VmError::StackUnderflow);
                        return Err(VmError::StackUnderflow);
                    }
                };
                self.pc = target;
                (target as u64, target)
            }
            Opcode::Push => {
                if self.stack.len() >= 1024 {
                    self.halted = true;
                    self.error = Some(VmError::StackOverflow);
                    return Err(VmError::StackOverflow);
                }
                self.stack.push(src1_val);
                self.pc += 1;
                (src1_val, cur_pc + 1)
            }
            Opcode::Pop => {
                let result = match self.stack.pop() {
                    Some(val) => val,
                    None => {
                        self.halted = true;
                        self.error = Some(VmError::StackUnderflow);
                        return Err(VmError::StackUnderflow);
                    }
                };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Eq => {
                let result = if src1_val == src2_val { 1 } else { 0 };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Neq => {
                let result = if src1_val != src2_val { 1 } else { 0 };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Lt => {
                let result = if src1_val < src2_val { 1 } else { 0 };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Gt => {
                let result = if src1_val > src2_val { 1 } else { 0 };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Lte => {
                let result = if src1_val <= src2_val { 1 } else { 0 };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Gte => {
                let result = if src1_val >= src2_val { 1 } else { 0 };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Assert => {
                if src1_val == 0 {
                    self.halted = true;
                    self.error = Some(VmError::AssertionFailed);
                    return Err(VmError::AssertionFailed);
                }
                self.pc += 1;
                (0, cur_pc + 1)
            }
            Opcode::SRead => {
                let slot = if inst.imm == -1 {
                    src2_val as i32
                } else {
                    inst.imm
                };
                let val = *self.storage.get(&slot).unwrap_or(&0);
                self.registers[dst_idx as usize] = val;
                self.pc += 1;
                (val, cur_pc + 1)
            }
            Opcode::SWrite => {
                let slot = if inst.imm == -1 {
                    src2_val as i32
                } else {
                    inst.imm
                };
                self.storage.insert(slot, src1_val);
                self.state_writes.push((slot, src1_val));
                self.pc += 1;
                (0, cur_pc + 1)
            }
            Opcode::Poseidon => {
                let result = poseidon4_hash(src1_val, src2_val);
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Log => {
                let val = src1_val;
                self.events.push(val);
                self.pc += 1;
                (0, cur_pc + 1)
            }
            Opcode::Syscall => {
                let result = match inst.imm {
                    1 => self.context.sender,
                    2 => self.context.block_height,
                    3 => self.context.nonce,
                    _ => 0,
                };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::VerifyMerkle => {
                let root = src1_val;
                let leaf = src2_val;
                let path_addr = inst.imm as usize;
                // Memory layout: [key: u64, 64 × sibling: u64]
                // Total: 520 bytes (65 × u64)
                let path_end = path_addr.wrapping_add(8 * 65);
                let result = if path_end <= self.memory.len() {
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(&self.memory[path_addr..path_addr + 8]);
                    let key = u64::from_le_bytes(bytes);

                    let mut current = leaf;
                    for i in 0..64 {
                        let sibling_addr = path_addr + 8 + i * 8;
                        bytes.copy_from_slice(&self.memory[sibling_addr..sibling_addr + 8]);
                        let sibling = u64::from_le_bytes(bytes);
                        let bit = (key >> i) & 1;
                        current = if bit == 0 {
                            poseidon4_hash(current, sibling)
                        } else {
                            poseidon4_hash(sibling, current)
                        };
                    }
                    if current == root {
                        1
                    } else {
                        0
                    }
                } else {
                    0
                };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
        };

        self.registers[0] = 0; // Enforce r0 is always 0

        self.trace.push(Step {
            pc: cur_pc,
            next_pc,
            instruction: inst,
            src1_idx,
            src2_idx,
            dst_idx,
            src1_val,
            src2_val,
            dst_val,
            registers: self.registers,
            memory_addr,
            memory_val,
            is_memory_write,
            stack_pointer: self.stack.len(),
        });

        debug!(
            pc = cur_pc,
            op = ?inst.opcode,
            rd = inst.rd,
            rs1 = inst.rs1,
            rs2 = inst.rs2,
            imm = inst.imm,
            dst_val,
            gas = self.gas_used,
            "Step executed"
        );

        Ok(())
    }

    pub fn run(&mut self, program: &[u64]) -> Result<ExecutionReceipt, VmError> {
        let receipt = self.run_receipt(program);
        if let Some(ref e) = receipt.error {
            Err(e.clone())
        } else {
            Ok(receipt)
        }
    }

    pub fn run_receipt(&mut self, program: &[u64]) -> ExecutionReceipt {
        let mut error = None;
        while !self.halted {
            if let Err(e) = self.step(program) {
                error = Some(e);
                break;
            }
        }

        let mut sorted_writes = self.state_writes.clone();
        sorted_writes.sort_by_key(|w| w.0);
        let mut bytes = Vec::new();
        for (slot, val) in sorted_writes {
            bytes.extend_from_slice(&slot.to_le_bytes());
            bytes.extend_from_slice(&val.to_le_bytes());
        }
        let mut state_writes_digest = [0u8; 32];
        if !bytes.is_empty() {
            use tiny_keccak::{Hasher, Keccak};
            let mut hasher = Keccak::v256();
            hasher.update(&bytes);
            hasher.finalize(&mut state_writes_digest);
        }

        ExecutionReceipt {
            success: error.is_none(),
            error: error.clone(),
            gas_used: self.gas_used,
            exit_code: if error.is_none() { 0 } else { 1 },
            events: self.events.clone(),
            final_pc: self.pc as u64,
            trace_len: self.trace.len() as u64,
            state_writes_digest,
        }
    }

    fn memory_word_addr(base: u64, imm: i32, memory_len: usize) -> Option<usize> {
        let addr = i128::from(base) + i128::from(imm);
        if addr < 0 {
            return None;
        }

        let addr = usize::try_from(addr).ok()?;
        let end = addr.checked_add(8)?;
        (end <= memory_len).then_some(addr)
    }

    pub fn gas_cost(opcode: Opcode) -> u64 {
        match opcode {
            Opcode::Halt => 0,
            Opcode::Load | Opcode::Store | Opcode::SRead | Opcode::SWrite => 3,
            Opcode::Poseidon | Opcode::VerifyMerkle => 10,
            Opcode::Call | Opcode::Ret | Opcode::Push | Opcode::Pop => 2,
            Opcode::Syscall => 5,
            _ => 1,
        }
    }
}

/// 4-round Poseidon hash over Goldilocks field (alpha=7, width=8, full rounds only).
/// Used for both VM execution and prover trace generation.
///
/// MDS circulant matrix first row: [7, 1, 3, 8, 8, 3, 4, 9]
/// Round constants: first 4 rounds from Plonky3 Poseidon1 Goldilocks width-8
pub fn poseidon4_hash(a: u64, b: u64) -> u64 {
    const P: u64 = 18446744069414584321;

    // MDS circulant matrix (8x8) from first row [7,1,3,8,8,3,4,9]
    const MDS: [[u64; 8]; 8] = [
        [7, 1, 3, 8, 8, 3, 4, 9],
        [9, 7, 1, 3, 8, 8, 3, 4],
        [4, 9, 7, 1, 3, 8, 8, 3],
        [3, 4, 9, 7, 1, 3, 8, 8],
        [8, 3, 4, 9, 7, 1, 3, 8],
        [8, 8, 3, 4, 9, 7, 1, 3],
        [3, 8, 8, 3, 4, 9, 7, 1],
        [1, 3, 8, 8, 3, 4, 9, 7],
    ];

    // Round constants (first 4 from Plonky3 Poseidon1 Goldilocks width-8)
    const RC: [[u64; 8]; 4] = [
        [
            0xdd5743e7f2a5a5d9,
            0xcb3a864e58ada44b,
            0xffa2449ed32f8cdc,
            0x42025f65d6bd13ee,
            0x7889175e25506323,
            0x34b98bb03d24b737,
            0xbdcc535ecc4faa2a,
            0x5b20ad869fc0d033,
        ],
        [
            0xf1dda5b9259dfcb4,
            0x27515210be112d59,
            0x4227d1718c766c3f,
            0x26d333161a5bd794,
            0x49b938957bf4b026,
            0x4a56b5938b213669,
            0x1120426b48c8353d,
            0x6b323c3f10a56cad,
        ],
        [
            0xce57d6245ddca6b2,
            0xb1fc8d402bba1eb1,
            0xb5c5096ca959bd04,
            0x6db55cd306d31f7f,
            0xc49d293a81cb9641,
            0x1ce55a4fe979719f,
            0xa92e60a9d178a4d1,
            0x002cc64973bcfd8c,
        ],
        [
            0xcea721cce82fb11b,
            0xe5b55eb8098ece81,
            0x4e30525c6f1ddd66,
            0x43c6702827070987,
            0xaca68430a7b5762a,
            0x3674238634df9c93,
            0x88cee1c825e33433,
            0xde99ae8d74b57176,
        ],
    ];

    let mut s: [u64; 8] = [a, b, 0, 0, 0, 0, 0, 0];

    for round_rc in RC.iter() {
        // Add round constants
        for i in 0..8 {
            s[i] = ((s[i] as u128 + round_rc[i] as u128) % P as u128) as u64;
        }
        // S-box: x^7 via x2=x^2, x4=x2^2, x7=x4*x2*x mod P
        let mut sbox: [u64; 8] = [0; 8];
        for i in 0..8 {
            let x = s[i];
            let x2 = ((x as u128 * x as u128) % P as u128) as u64;
            let x4 = ((x2 as u128 * x2 as u128) % P as u128) as u64;
            sbox[i] = (((x4 as u128 * x2 as u128) % P as u128 * x as u128) % P as u128) as u64;
        }
        // MDS linear layer
        let mut next: [u64; 8] = [0; 8];
        for i in 0..8 {
            let mut sum: u128 = 0;
            for j in 0..8 {
                sum = (sum + MDS[i][j] as u128 * sbox[j] as u128) % P as u128;
            }
            next[i] = sum as u64;
        }
        s = next;
    }

    s[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst(opcode: Opcode, rd: u8, rs1: u8, rs2: u8, imm: i32) -> u64 {
        Instruction {
            opcode,
            rd,
            rs1,
            rs2,
            imm,
        }
        .encode()
    }

    #[test]
    fn push_and_pop_round_trip_through_stack() {
        let program = vec![
            inst(Opcode::Push, 0, 1, 0, 0),
            inst(Opcode::Pop, 2, 0, 0, 0),
            inst(Opcode::Halt, 0, 0, 0, 0),
        ];

        let mut vm = Vm::new(64);
        vm.registers[1] = 42;
        let receipt = vm.run_receipt(&program);

        assert!(receipt.success);
        assert_eq!(vm.registers[2], 42);
        assert!(vm.stack.is_empty());
    }

    #[test]
    fn call_and_ret_use_return_stack() {
        let program = vec![
            inst(Opcode::Call, 0, 0, 0, 2),
            inst(Opcode::Halt, 0, 0, 0, 0),
            inst(Opcode::Load, 1, 0, 0, 7),
            inst(Opcode::Ret, 0, 0, 0, 0),
        ];

        let mut vm = Vm::new(64);
        let receipt = vm.run_receipt(&program);

        assert!(receipt.success);
        assert_eq!(vm.registers[1], 7);
        assert_eq!(vm.pc, 1);
        assert!(vm.stack.is_empty());
    }

    #[test]
    fn gas_limit_stops_unbounded_execution() {
        let program = vec![inst(Opcode::Jmp, 0, 0, 0, 0)];
        let mut vm = Vm::with_gas_limit(64, 3);

        let receipt = vm.run_receipt(&program);
        assert!(!receipt.success);
        assert_eq!(receipt.error, Some(VmError::OutOfGas));
    }

    #[test]
    fn gas_accounting_matches_instruction_costs() {
        let program = vec![
            inst(Opcode::Load, 1, 0, 0, 9),
            inst(Opcode::Push, 0, 1, 0, 0),
            inst(Opcode::Syscall, 2, 0, 0, 1),
            inst(Opcode::Halt, 0, 0, 0, 0),
        ];

        let mut vm = Vm::new(64);
        vm.context.sender = 77;
        let receipt = vm.run_receipt(&program);

        assert!(receipt.success);
        assert_eq!(vm.gas_used, 10);
        assert_eq!(vm.registers[1], 9);
        assert_eq!(vm.registers[2], 77);
        assert_eq!(vm.trace.len(), 4);
    }

    #[test]
    fn step_after_halt_is_idempotent() {
        let program = vec![
            inst(Opcode::Halt, 0, 0, 0, 0),
            inst(Opcode::Load, 1, 0, 0, 99),
        ];

        let mut vm = Vm::new(64);
        let _ = vm.step(&program);

        assert!(vm.halted);
        assert_eq!(vm.pc, 0);
        assert_eq!(vm.trace.len(), 1);

        let _ = vm.step(&program);

        assert!(vm.halted);
        assert_eq!(vm.pc, 0);
        assert_eq!(vm.trace.len(), 1);
        assert_eq!(vm.registers[1], 0);
    }

    #[test]
    fn test_memory_oob_safety() {
        let program_load_oob = vec![inst(Opcode::Load, 1, 1, 0, 100)];
        let mut vm = Vm::new(64);
        let receipt = vm.run_receipt(&program_load_oob);
        assert!(!receipt.success);
        assert_eq!(receipt.error, Some(VmError::InvalidMemoryAccess));

        let program_store_oob = vec![inst(Opcode::Store, 0, 1, 2, 100)];
        let mut vm2 = Vm::new(64);
        let receipt2 = vm2.run_receipt(&program_store_oob);
        assert!(!receipt2.success);
        assert_eq!(receipt2.error, Some(VmError::InvalidMemoryAccess));
    }
}
