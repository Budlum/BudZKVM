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
                } else {
                    let addr = (src1_val as i64 + inst.imm as i64) as usize;
                    if addr + 8 <= self.memory.len() {
                        let mut bytes = [0u8; 8];
                        bytes.copy_from_slice(&self.memory[addr..addr + 8]);
                        u64::from_le_bytes(bytes)
                    } else {
                        0
                    }
                };
                self.registers[dst_idx as usize] = result;
                self.pc += 1;
                (result, cur_pc + 1)
            }
            Opcode::Store => {
                let addr = (src1_val as i64 + inst.imm as i64) as usize;
                if addr + 8 <= self.memory.len() {
                    let bytes = src2_val.to_le_bytes();
                    self.memory[addr..addr + 8].copy_from_slice(&bytes);
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
                let path = self.registers[inst.imm as usize];
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
        });
    }

    pub fn run(&mut self, program: &[u64]) {
        while !self.halted {
            self.step(program);
        }
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
}
