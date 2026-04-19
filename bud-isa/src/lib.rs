#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    Halt = 0x00,
    Add = 0x01,
    Sub = 0x02,
    Mul = 0x03,
    Div = 0x04,
    Inv = 0x05,
    And = 0x06,
    Or = 0x07,
    Xor = 0x08,
    Not = 0x09,
    Eq = 0x0A,
    Neq = 0x0B,
    Lt = 0x0C,
    Gt = 0x0D,
    Lte = 0x0E,
    Gte = 0x0F,
    Jmp = 0x10,
    Jnz = 0x11,
    Call = 0x12,
    Ret = 0x13,
    Load = 0x14,
    Store = 0x15,
    Push = 0x16,
    Pop = 0x17,
    Assert = 0x18,
    Poseidon = 0x19,
    Log = 0x1A,
    SRead = 0x1B,
    SWrite = 0x1C,
    Syscall = 0x1D,
    VerifyMerkle = 0x1E,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    pub opcode: Opcode,
    pub rd: u8,
    pub rs1: u8,
    pub rs2: u8,
    pub imm: i32,
}

impl Instruction {
    pub fn encode(&self) -> u64 {
        let mut res = self.opcode as u64;
        res |= (self.rd as u64) << 8;
        res |= (self.rs1 as u64) << 13;
        res |= (self.rs2 as u64) << 18;
        res |= ((self.imm as u32) as u64) << 23;
        res
    }

    pub fn decode(val: u64) -> Self {
        let op_u8 = (val & 0xFF) as u8;
        let opcode = match op_u8 {
            0x00 => Opcode::Halt,
            0x01 => Opcode::Add,
            0x02 => Opcode::Sub,
            0x03 => Opcode::Mul,
            0x04 => Opcode::Div,
            0x05 => Opcode::Inv,
            0x06 => Opcode::And,
            0x07 => Opcode::Or,
            0x08 => Opcode::Xor,
            0x09 => Opcode::Not,
            0x0A => Opcode::Eq,
            0x0B => Opcode::Neq,
            0x0C => Opcode::Lt,
            0x0D => Opcode::Gt,
            0x0E => Opcode::Lte,
            0x0F => Opcode::Gte,
            0x10 => Opcode::Jmp,
            0x11 => Opcode::Jnz,
            0x12 => Opcode::Call,
            0x13 => Opcode::Ret,
            0x14 => Opcode::Load,
            0x15 => Opcode::Store,
            0x16 => Opcode::Push,
            0x17 => Opcode::Pop,
            0x18 => Opcode::Assert,
            0x19 => Opcode::Poseidon,
            0x1A => Opcode::Log,
            0x1B => Opcode::SRead,
            0x1C => Opcode::SWrite,
            0x1D => Opcode::Syscall,
            0x1E => Opcode::VerifyMerkle,
            _ => Opcode::Halt,
        };
        let rd = ((val >> 8) & 0x1F) as u8;
        let rs1 = ((val >> 13) & 0x1F) as u8;
        let rs2 = ((val >> 18) & 0x1F) as u8;
        let imm = ((val >> 23) & 0xFFFFFFFF) as i32;
        
        Self {
            opcode,
            rd,
            rs1,
            rs2,
            imm,
        }
    }
}
