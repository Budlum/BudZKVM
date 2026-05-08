use bud_isa::{Instruction, Opcode};

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
        }
    }

    pub fn consume_gas(&mut self, amount: u64) {
        self.gas_used = self.gas_used.saturating_add(amount);
        if self.gas_used > self.gas_limit {
            self.halted = true;
            panic!(
                "Out of gas: used {}, limit {}",
                self.gas_used, self.gas_limit
            );
        }
    }

    pub fn step(&mut self, program: &[u64]) {
        if self.halted || self.pc >= program.len() {
            self.halted = true;
            return;
        }

        let raw_inst = program[self.pc];
        let inst = Instruction::decode(raw_inst);
        let cur_pc = self.pc;
        self.consume_gas(Self::gas_cost(inst.opcode));

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
                let result = if src2_val != 0 {
                    src1_val / src2_val
                } else {
                    0
                };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Inv => {
                let result = !src1_val;
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
                    0
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
                let target = (cur_pc as i64 + inst.imm as i64) as usize;
                self.stack.push((cur_pc + 1) as u64);
                self.pc = target;
                ((cur_pc + 1) as u64, target)
            }
            Opcode::Ret => {
                let target = self.stack.pop().expect("Return stack underflow") as usize;
                self.pc = target;
                (target as u64, target)
            }
            Opcode::Push => {
                self.stack.push(src1_val);
                self.pc += 1;
                (src1_val, cur_pc + 1)
            }
            Opcode::Pop => {
                let result = self.stack.pop().expect("Stack underflow");
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
                    panic!("Assertion failed at PC {}", cur_pc);
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
                self.pc += 1;
                (0, cur_pc + 1)
            }
            Opcode::Poseidon => {
                let result = src1_val
                    .wrapping_mul(31)
                    .wrapping_add(src2_val)
                    .wrapping_add(0x1337);
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
                let path = usize::try_from(inst.imm)
                    .ok()
                    .and_then(|idx| self.registers.get(idx))
                    .copied()
                    .unwrap_or(0);
                let computed = leaf
                    .wrapping_mul(31)
                    .wrapping_add(path)
                    .wrapping_add(0x1337);
                let result = if computed == root { 1 } else { 0 };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
        };

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
    }

    pub fn run(&mut self, program: &[u64]) {
        while !self.halted {
            self.step(program);
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

    fn gas_cost(opcode: Opcode) -> u64 {
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
        vm.run(&program);

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
        vm.run(&program);

        assert_eq!(vm.registers[1], 7);
        assert_eq!(vm.pc, 1);
        assert!(vm.stack.is_empty());
    }

    #[test]
    #[should_panic(expected = "Out of gas")]
    fn gas_limit_stops_unbounded_execution() {
        let program = vec![inst(Opcode::Jmp, 0, 0, 0, 0)];
        let mut vm = Vm::with_gas_limit(64, 3);

        vm.run(&program);
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
        vm.run(&program);

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
        vm.step(&program);

        assert!(vm.halted);
        assert_eq!(vm.pc, 0);
        assert_eq!(vm.trace.len(), 1);

        vm.step(&program);

        assert!(vm.halted);
        assert_eq!(vm.pc, 0);
        assert_eq!(vm.trace.len(), 1);
        assert_eq!(vm.registers[1], 0);
    }

    #[test]
    fn pc_outside_program_halts_without_trace_row() {
        let program = vec![inst(Opcode::Halt, 0, 0, 0, 0)];
        let mut vm = Vm::new(64);
        vm.pc = program.len();

        vm.step(&program);

        assert!(vm.halted);
        assert_eq!(vm.trace.len(), 0);
    }

    #[test]
    fn invalid_memory_accesses_are_zero_or_noop() {
        let load_out_of_bounds = inst(Opcode::Load, 1, 2, 0, 5);
        let load_negative = inst(Opcode::Load, 3, 2, 0, -1);
        let store_out_of_bounds = inst(Opcode::Store, 0, 2, 4, 5);
        let store_negative = inst(Opcode::Store, 0, 2, 4, -1);
        let program = vec![
            load_out_of_bounds,
            load_negative,
            store_out_of_bounds,
            store_negative,
            inst(Opcode::Halt, 0, 0, 0, 0),
        ];

        let mut vm = Vm::new(8);
        vm.registers[2] = 0;
        vm.registers[4] = 0xAABB_CCDD_EEFF_0011;
        vm.run(&program);

        assert_eq!(vm.registers[1], 0);
        assert_eq!(vm.registers[3], 0);
        assert_eq!(vm.memory, vec![0; 8]);
    }

    #[test]
    fn verify_merkle_with_invalid_path_register_returns_false() {
        let program = vec![
            inst(Opcode::VerifyMerkle, 3, 1, 2, 99),
            inst(Opcode::VerifyMerkle, 4, 1, 2, -1),
            inst(Opcode::Halt, 0, 0, 0, 0),
        ];

        let mut vm = Vm::new(64);
        vm.registers[1] = 123;
        vm.registers[2] = 5;
        vm.run(&program);

        assert_eq!(vm.registers[3], 0);
        assert_eq!(vm.registers[4], 0);
        assert_eq!(vm.trace.len(), 3);
    }

    #[test]
    fn branch_and_jump_edge_cases_are_deterministic() {
        let program = vec![
            inst(Opcode::Jnz, 0, 1, 0, 3),
            inst(Opcode::Load, 2, 0, 0, 11),
            inst(Opcode::Jmp, 0, 0, 0, 3),
            inst(Opcode::Load, 2, 0, 0, 22),
            inst(Opcode::Halt, 0, 0, 0, 0),
        ];

        let mut vm = Vm::new(64);
        vm.run(&program);

        assert_eq!(vm.registers[2], 11);
        assert!(vm.halted);
        assert_eq!(vm.pc, 5);
        assert_eq!(vm.trace.len(), 3);
        assert_eq!(vm.trace[0].next_pc, 1);
        assert_eq!(vm.trace[2].next_pc, 5);

        let mut taken = Vm::new(64);
        taken.registers[1] = 1;
        taken.run(&program);

        assert_eq!(taken.registers[2], 22);
        assert!(taken.halted);
        assert_eq!(taken.pc, 4);
        assert_eq!(taken.trace.len(), 3);
        assert_eq!(taken.trace[0].next_pc, 3);
    }

    #[test]
    fn arithmetic_overflow_wraps_modulo_u64() {
        let program = vec![
            inst(Opcode::Add, 3, 1, 2, 0),
            inst(Opcode::Sub, 4, 0, 2, 0),
            inst(Opcode::Mul, 5, 1, 2, 0),
            inst(Opcode::Halt, 0, 0, 0, 0),
        ];

        let mut vm = Vm::new(64);
        vm.registers[1] = u64::MAX;
        vm.registers[2] = 2;
        vm.run(&program);

        assert_eq!(vm.registers[3], 1);
        assert_eq!(vm.registers[4], u64::MAX - 1);
        assert_eq!(vm.registers[5], u64::MAX - 1);
    }
}
