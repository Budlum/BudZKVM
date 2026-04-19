use bud_isa::{Opcode, Instruction};

pub struct Vm {
    pub registers: [u64; 32],
    pub pc: usize,
    pub memory: Vec<u8>,
    pub storage: std::collections::HashMap<i32, u64>,
    pub events: Vec<u64>,
    pub context: Context,
    pub trace: Vec<Step>,
    pub halted: bool,
}

pub struct Context {
    pub sender: u64,
    pub block_height: u64,
}

#[derive(Debug, Clone)]
pub struct Step {
    pub pc: usize,
    pub registers: [u64; 32],
    pub instruction: Instruction,
}

impl Vm {
    pub fn new(memory_size: usize) -> Self {
        Self {
            registers: [0; 32],
            pc: 0,
            memory: vec![0; memory_size],
            storage: std::collections::HashMap::new(),
            events: Vec::new(),
            context: Context { sender: 0, block_height: 0 },
            trace: Vec::new(),
            halted: false,
        }
    }

    pub fn step(&mut self, program: &[u64]) {
        if self.halted || self.pc >= program.len() {
            self.halted = true;
            return;
        }

        let raw_inst = program[self.pc];
        let inst = Instruction::decode(raw_inst);

        self.trace.push(Step {
            pc: self.pc,
            registers: self.registers,
            instruction: inst,
        });

        match inst.opcode {
            Opcode::Halt => {
                self.halted = true;
            }
            Opcode::Add => {
                self.registers[inst.rd as usize] = self.registers[inst.rs1 as usize].wrapping_add(self.registers[inst.rs2 as usize]);
                self.pc += 1;
            }
            Opcode::Sub => {
                self.registers[inst.rd as usize] = self.registers[inst.rs1 as usize].wrapping_sub(self.registers[inst.rs2 as usize]);
                self.pc += 1;
            }
            Opcode::Mul => {
                self.registers[inst.rd as usize] = self.registers[inst.rs1 as usize].wrapping_mul(self.registers[inst.rs2 as usize]);
                self.pc += 1;
            }
            Opcode::Load => {
                if inst.rs1 == 0 {
                    self.registers[inst.rd as usize] = inst.imm as u64;
                } else {
                    let addr = (self.registers[inst.rs1 as usize] as i64 + inst.imm as i64) as usize;
                    if addr + 8 <= self.memory.len() {
                        let mut bytes = [0u8; 8];
                        bytes.copy_from_slice(&self.memory[addr..addr+8]);
                        self.registers[inst.rd as usize] = u64::from_le_bytes(bytes);
                    }
                }
                self.pc += 1;
            }
            Opcode::Store => {
                let addr = (self.registers[inst.rs1 as usize] as i64 + inst.imm as i64) as usize;
                if addr + 8 <= self.memory.len() {
                    let bytes = self.registers[inst.rs2 as usize].to_le_bytes();
                    self.memory[addr..addr+8].copy_from_slice(&bytes);
                }
                self.pc += 1;
            }
            Opcode::Jmp => {
                self.pc = (self.pc as i64 + inst.imm as i64) as usize;
            }
            Opcode::Jnz => {
                if self.registers[inst.rs1 as usize] != 0 {
                    self.pc = (self.pc as i64 + inst.imm as i64) as usize;
                } else {
                    self.pc += 1;
                }
            }
            Opcode::Eq => {
                self.registers[inst.rd as usize] = if self.registers[inst.rs1 as usize] == self.registers[inst.rs2 as usize] { 1 } else { 0 };
                self.pc += 1;
            }
            Opcode::Neq => {
                self.registers[inst.rd as usize] = if self.registers[inst.rs1 as usize] != self.registers[inst.rs2 as usize] { 1 } else { 0 };
                self.pc += 1;
            }
            Opcode::Lt => {
                self.registers[inst.rd as usize] = if self.registers[inst.rs1 as usize] < self.registers[inst.rs2 as usize] { 1 } else { 0 };
                self.pc += 1;
            }
            Opcode::Gt => {
                self.registers[inst.rd as usize] = if self.registers[inst.rs1 as usize] > self.registers[inst.rs2 as usize] { 1 } else { 0 };
                self.pc += 1;
            }
            Opcode::Lte => {
                self.registers[inst.rd as usize] = if self.registers[inst.rs1 as usize] <= self.registers[inst.rs2 as usize] { 1 } else { 0 };
                self.pc += 1;
            }
            Opcode::Gte => {
                self.registers[inst.rd as usize] = if self.registers[inst.rs1 as usize] >= self.registers[inst.rs2 as usize] { 1 } else { 0 };
                self.pc += 1;
            }
            Opcode::Assert => {
                if self.registers[inst.rs1 as usize] == 0 {
                    panic!("Assertion failed at PC {}", self.pc);
                }
                self.pc += 1;
            }
            Opcode::SRead => {
                let slot = if inst.imm == -1 { self.registers[inst.rs2 as usize] as i32 } else { inst.imm };
                let val = *self.storage.get(&slot).unwrap_or(&0);
                self.registers[inst.rd as usize] = val;
                self.pc += 1;
            }
            Opcode::SWrite => {
                let slot = if inst.imm == -1 { self.registers[inst.rs2 as usize] as i32 } else { inst.imm };
                let val = self.registers[inst.rs1 as usize];
                self.storage.insert(slot, val);
                self.pc += 1;
            }
            Opcode::Poseidon => {
                let v1 = self.registers[inst.rs1 as usize];
                let v2 = self.registers[inst.rs2 as usize];
                self.registers[inst.rd as usize] = v1.wrapping_mul(31).wrapping_add(v2).wrapping_add(0x1337);
                self.pc += 1;
            }
            Opcode::Log => {
                let val = self.registers[inst.rs1 as usize];
                self.events.push(val);
                self.pc += 1;
            }
            Opcode::Syscall => {
                match inst.imm {
                    1 => self.registers[inst.rd as usize] = self.context.sender,
                    2 => self.registers[inst.rd as usize] = self.context.block_height,
                    _ => {}
                }
                self.pc += 1;
            }
            Opcode::VerifyMerkle => {
                let root = self.registers[inst.rs1 as usize];
                let leaf = self.registers[inst.rs2 as usize];
                let path = self.registers[inst.imm as usize];
                let computed = leaf.wrapping_mul(31).wrapping_add(path).wrapping_add(0x1337);
                self.registers[inst.rd as usize] = if computed == root { 1 } else { 0 };
                self.pc += 1;
            }
            _ => {
                self.pc += 1;
            }
        }
    }

    pub fn run(&mut self, program: &[u64]) {
        while !self.halted {
            self.step(program);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bud_isa::{Opcode, Instruction};

    #[test]
    fn test_add() {
        let mut vm = Vm::new(1024);
        let prog = vec![
            Instruction { opcode: Opcode::Load, rd: 1, rs1: 0, rs2: 0, imm: 10 }.encode(),
            Instruction { opcode: Opcode::Load, rd: 2, rs1: 0, rs2: 0, imm: 20 }.encode(),
            Instruction { opcode: Opcode::Add, rd: 3, rs1: 1, rs2: 2, imm: 0 }.encode(),
            Instruction { opcode: Opcode::Halt, rd: 0, rs1: 0, rs2: 0, imm: 0 }.encode(),
        ];
        vm.run(&prog);
        assert_eq!(vm.registers[3], 30);
    }

    #[test]
    fn test_storage() {
        let mut vm = Vm::new(1024);
        let prog = vec![
            Instruction { opcode: Opcode::Load, rd: 1, rs1: 0, rs2: 0, imm: 42 }.encode(),
            Instruction { opcode: Opcode::SWrite, rd: 0, rs1: 1, rs2: 0, imm: 5 }.encode(),
            Instruction { opcode: Opcode::SRead, rd: 2, rs1: 0, rs2: 0, imm: 5 }.encode(),
            Instruction { opcode: Opcode::Halt, rd: 0, rs1: 0, rs2: 0, imm: 0 }.encode(),
        ];
        vm.run(&prog);
        assert_eq!(vm.registers[2], 42);
    }
}
