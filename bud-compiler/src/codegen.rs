use crate::ast::*;
use bud_isa::{Opcode, Instruction};

pub struct Codegen {
    instructions: Vec<u64>,
    next_reg: u8,
}

impl Codegen {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            next_reg: 1,
        }
    }

    pub fn generate(&mut self, contract: &Contract) -> Vec<u64> {
        for func in &contract.functions {
            self.generate_function(func, contract);
        }
        self.emit(Opcode::Halt, 0, 0, 0, 0);
        self.instructions.clone()
    }

    fn generate_function(&mut self, func: &Function, contract: &Contract) {
        let mut scope = std::collections::HashMap::new();
        for param in &func.params {
            let reg = self.alloc_reg();
            scope.insert(param.name.clone(), reg);
        }
        let mut storage_map = std::collections::HashMap::new();
        for (i, field) in contract.storage.iter().enumerate() {
            storage_map.insert(field.name.clone(), i as i32);
        }

        for stmt in &func.body {
            self.generate_stmt(stmt, &mut scope, &storage_map, contract);
        }
    }

    fn generate_stmt(&mut self, stmt: &Stmt, scope: &mut std::collections::HashMap<String, u8>, storage: &std::collections::HashMap<String, i32>, contract: &Contract) {
        match stmt {
            Stmt::Let(name, expr) => {
                let reg = self.generate_expr(expr, scope, storage);
                scope.insert(name.clone(), reg);
            }
            Stmt::Constrain(expr) => {
                let reg = self.generate_expr(expr, scope, storage);
                self.emit(Opcode::Assert, 0, reg, 0, 0);
            }
            Stmt::StorageWrite(name, expr) => {
                let reg = self.generate_expr(expr, scope, storage);
                let slot = *storage.get(name).expect("Unknown storage variable");
                self.emit(Opcode::SWrite, 0, reg, 0, slot);
            }
            Stmt::MappingWrite(name, key, val) => {
                let base_slot = *storage.get(name).expect("Unknown mapping");
                let key_reg = self.generate_expr(key, scope, storage);
                let val_reg = self.generate_expr(val, scope, storage);
                
                let base_reg = self.alloc_reg();
                self.emit(Opcode::Load, base_reg, 0, 0, base_slot);
                
                let target_slot_reg = self.alloc_reg();
                self.emit(Opcode::Poseidon, target_slot_reg, base_reg, key_reg, 0);
                
                self.emit(Opcode::SWrite, 0, val_reg, target_slot_reg, -1); 
            }
            Stmt::If(cond, then_branch, else_branch) => {
                let cond_reg = self.generate_expr(cond, scope, storage);
                let jump_to_then_idx = self.instructions.len();
                self.emit(Opcode::Jnz, 0, cond_reg, 0, 0);
                
                if let Some(eb) = else_branch {
                    for s in eb { self.generate_stmt(s, scope, storage, contract); }
                }
                let jump_to_end_idx = self.instructions.len();
                self.emit(Opcode::Jmp, 0, 0, 0, 0);
                
                let then_start_idx = self.instructions.len();
                for s in then_branch { self.generate_stmt(s, scope, storage, contract); }
                let end_idx = self.instructions.len();
                
                self.patch_jump(jump_to_then_idx, (then_start_idx as i32) - (jump_to_then_idx as i32));
                self.patch_jump(jump_to_end_idx, (end_idx as i32) - (jump_to_end_idx as i32));
            }
            Stmt::Emit(_name, args) => {
                if !args.is_empty() {
                    let reg = self.generate_expr(&args[0], scope, storage);
                    self.emit(Opcode::Log, 0, reg, 0, 0);
                }
            }
            Stmt::Expr(expr) => {
                self.generate_expr(expr, scope, storage);
            }
            _ => {}
        }
    }

    fn patch_jump(&mut self, idx: usize, offset: i32) {
        let inst_raw = self.instructions[idx];
        let mut inst = Instruction::decode(inst_raw);
        inst.imm = offset;
        self.instructions[idx] = inst.encode();
    }

    fn generate_expr(&mut self, expr: &Expr, scope: &std::collections::HashMap<String, u8>, storage: &std::collections::HashMap<String, i32>) -> u8 {
        match expr {
            Expr::Int(val) => {
                let reg = self.alloc_reg();
                self.emit(Opcode::Load, reg, 0, 0, *val as i32);
                reg
            }
            Expr::Ident(name) => {
                *scope.get(name).expect("Undefined variable in codegen")
            }
            Expr::StorageRead(name) => {
                let reg = self.alloc_reg();
                let slot = *storage.get(name).expect("Unknown storage variable");
                self.emit(Opcode::SRead, reg, 0, 0, slot);
                reg
            }
            Expr::MappingRead(name, key) => {
                let base_slot = *storage.get(name).expect("Unknown mapping");
                let key_reg = self.generate_expr(key, scope, storage);
                
                let base_reg = self.alloc_reg();
                self.emit(Opcode::Load, base_reg, 0, 0, base_slot);
                
                let target_slot_reg = self.alloc_reg();
                self.emit(Opcode::Poseidon, target_slot_reg, base_reg, key_reg, 0);
                
                let res_reg = self.alloc_reg();
                self.emit(Opcode::SRead, res_reg, 0, target_slot_reg, -1);
                res_reg
            }
            Expr::Binary(left, op, right) => {
                let l_reg = self.generate_expr(left, scope, storage);
                let r_reg = self.generate_expr(right, scope, storage);
                let res_reg = self.alloc_reg();
                
                let opcode = match op {
                    BinOp::Add => Opcode::Add,
                    BinOp::Sub => Opcode::Sub,
                    BinOp::Mul => Opcode::Mul,
                    BinOp::Div => Opcode::Div,
                    BinOp::Eq => Opcode::Eq,
                    BinOp::Neq => Opcode::Neq,
                    BinOp::Lt => Opcode::Lt,
                    BinOp::Gt => Opcode::Gt,
                    BinOp::Lte => Opcode::Lte,
                    BinOp::Gte => Opcode::Gte,
                };
                
                self.emit(opcode, res_reg, l_reg, r_reg, 0);
                res_reg
            }
            Expr::Call(name, args) => {
                if name == "poseidon" {
                    let r1 = self.generate_expr(&args[0], scope, storage);
                    let r2 = self.generate_expr(&args[1], scope, storage);
                    let res = self.alloc_reg();
                    self.emit(Opcode::Poseidon, res, r1, r2, 0);
                    res
                } else if name == "msg::sender" {
                    let res = self.alloc_reg();
                    self.emit(Opcode::Syscall, res, 0, 0, 1);
                    res
                } else if name == "msg::nonce" {
                    let res = self.alloc_reg();
                    self.emit(Opcode::Syscall, res, 0, 0, 3);
                    res
                } else if name == "block::number" {
                    let res = self.alloc_reg();
                    self.emit(Opcode::Syscall, res, 0, 0, 2);
                    res
                } else if name == "verify_merkle_proof" {
                    let r_root = self.generate_expr(&args[0], scope, storage);
                    let r_leaf = self.generate_expr(&args[1], scope, storage);
                    let r_path = self.generate_expr(&args[2], scope, storage);
                    let res = self.alloc_reg();
                    self.emit(Opcode::VerifyMerkle, res, r_root, r_leaf, r_path as i32);
                    res
                } else {
                    0
                }
            }
        }
    }

    fn alloc_reg(&mut self) -> u8 {
        let r = self.next_reg;
        self.next_reg += 1;
        r
    }

    fn emit(&mut self, opcode: Opcode, rd: u8, rs1: u8, rs2: u8, imm: i32) {
        let inst = Instruction {
            opcode,
            rd,
            rs1,
            rs2,
            imm,
        };
        self.instructions.push(inst.encode());
    }
}
