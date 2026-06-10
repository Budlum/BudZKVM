use crate::ast::*;
use crate::CompileError;
use bud_isa::{Instruction, IsaProfile, Opcode};

#[allow(dead_code)]
pub struct Codegen {
    instructions: Vec<u64>,
    next_reg: u8,
    profile: IsaProfile,
    error: Option<CompileError>,
    unpatched_calls: Vec<(usize, String)>,
    struct_layouts: std::collections::HashMap<String, Vec<String>>,
}

impl Default for Codegen {
    fn default() -> Self {
        Self::new()
    }
}

impl Codegen {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            next_reg: 1,
            profile: IsaProfile::Production,
            error: None,
            unpatched_calls: Vec::new(),
            struct_layouts: std::collections::HashMap::new(),
        }
    }

    pub fn new_with_profile(profile: IsaProfile) -> Self {
        Self {
            instructions: Vec::new(),
            next_reg: 1,
            profile,
            error: None,
            unpatched_calls: Vec::new(),
            struct_layouts: std::collections::HashMap::new(),
        }
    }

    pub fn generate(&mut self, contract: &Contract) -> Result<Vec<u64>, CompileError> {
        // Populate struct layouts
        for s in &contract.structs {
            let mut fields = Vec::new();
            for f in &s.fields {
                fields.push(f.name.clone());
            }
            self.struct_layouts.insert(s.name.clone(), fields);
        }

        self.emit(Opcode::Load, 31, 0, 0, 4096); // Initialize heap ptr!
        let jump_to_main_idx = self.instructions.len();
        self.emit(Opcode::Call, 0, 0, 0, 0);
        self.emit(Opcode::Halt, 0, 0, 0, 0);

        let mut func_offsets = std::collections::HashMap::new();

        for func in &contract.functions {
            func_offsets.insert(func.name.clone(), self.instructions.len());
            self.generate_function(func, contract);
        }

        // Patch main call
        if let Some(main_idx) = func_offsets.get("main") {
            self.patch_jump(jump_to_main_idx, (*main_idx as i32) - (jump_to_main_idx as i32));
        } else {
            self.error = Some(CompileError::CodegenError("main function not found".to_string()));
        }

        let unpatched = std::mem::take(&mut self.unpatched_calls);
        for (call_idx, func_name) in unpatched {
            if let Some(target_idx) = func_offsets.get(&func_name) {
                self.patch_jump(call_idx, (*target_idx as i32) - (call_idx as i32));
            } else {
                self.error = Some(CompileError::CodegenError(format!("Undefined function {}", func_name)));
            }
        }

        if let Some(err) = self.error.take() {
            Err(err)
        } else {
            Ok(self.instructions.clone())
        }
    }

    fn generate_function(&mut self, func: &Function, contract: &Contract) {
        if self.error.is_some() {
            return;
        }

        self.next_reg = 1;
        let mut scope = std::collections::HashMap::new();
        
        let ret_addr_reg = self.alloc_reg();
        self.emit(Opcode::Pop, ret_addr_reg, 0, 0, 0);

        let mut param_regs = Vec::new();
        for _ in 0..func.params.len() {
            param_regs.push(self.alloc_reg());
        }

        for param_reg in param_regs.iter().rev() {
            self.emit(Opcode::Pop, *param_reg, 0, 0, 0);
        }

        for (param, reg) in func.params.iter().zip(param_regs.iter()) {
            scope.insert(param.name.clone(), *reg);
        }

        self.emit(Opcode::Push, 0, ret_addr_reg, 0, 0);

        let mut storage_map = std::collections::HashMap::new();
        for (i, field) in contract.storage.iter().enumerate() {
            storage_map.insert(field.name.clone(), i as i32);
        }

        for stmt in &func.body {
            self.generate_stmt(stmt, &mut scope, &storage_map);
        }

        let temp = self.alloc_reg();
        self.emit(Opcode::Pop, temp, 0, 0, 0);
        let zero = self.alloc_reg();
        self.emit(Opcode::Load, zero, 0, 0, 0);
        self.emit(Opcode::Push, 0, zero, 0, 0);
        self.emit(Opcode::Push, 0, temp, 0, 0);
        self.emit(Opcode::Ret, 0, 0, 0, 0);
    }

    fn generate_stmt(
        &mut self,
        stmt: &Stmt,
        scope: &mut std::collections::HashMap<String, u8>,
        storage: &std::collections::HashMap<String, i32>,
    ) {
        if self.error.is_some() {
            return;
        }

        let saved_reg = self.next_reg;

        match stmt {
            Stmt::Let(name, expr) => {
                let reg = self.generate_expr(expr, scope, storage);
                scope.insert(name.clone(), reg);
                if reg >= saved_reg {
                    self.next_reg = reg + 1;
                } else {
                    self.next_reg = saved_reg;
                }
            }
            Stmt::Constrain(expr) => {
                let reg = self.generate_expr(expr, scope, storage);
                self.emit(Opcode::Assert, 0, reg, 0, 0);
                self.next_reg = saved_reg;
            }
            Stmt::StorageWrite(name, expr) => {
                let reg = self.generate_expr(expr, scope, storage);
                let slot = match storage.get(name) {
                    Some(s) => *s,
                    None => {
                        if self.error.is_none() {
                            self.error = Some(CompileError::CodegenError(format!(
                                "Unknown storage variable: {}",
                                name
                            )));
                        }
                        return;
                    }
                };
                self.emit(Opcode::SWrite, 0, reg, 0, slot);
                self.next_reg = saved_reg;
            }
            Stmt::MappingWrite(name, key, val) => {
                let base_slot = match storage.get(name) {
                    Some(s) => *s,
                    None => {
                        if self.error.is_none() {
                            self.error = Some(CompileError::CodegenError(format!(
                                "Unknown mapping variable: {}",
                                name
                            )));
                        }
                        return;
                    }
                };
                let key_reg = self.generate_expr(key, scope, storage);
                let val_reg = self.generate_expr(val, scope, storage);

                let base_reg = self.alloc_reg();
                self.emit(Opcode::Load, base_reg, 0, 0, base_slot);

                let target_slot_reg = self.alloc_reg();
                self.emit(Opcode::Poseidon, target_slot_reg, base_reg, key_reg, 0);

                self.emit(Opcode::SWrite, 0, val_reg, target_slot_reg, -1);
                self.next_reg = saved_reg;
            }
            Stmt::Assign(name, expr) => {
                let reg = self.generate_expr(expr, scope, storage);
                let target_reg = match scope.get(name) {
                    Some(r) => *r,
                    None => {
                        if self.error.is_none() {
                            self.error = Some(CompileError::CodegenError(format!(
                                "Undefined variable: {}",
                                name
                            )));
                        }
                        return;
                    }
                };
                self.emit(Opcode::Add, target_reg, reg, 0, 0);
                self.next_reg = saved_reg;
            }
            Stmt::If(cond, then_branch, else_branch) => {
                let cond_reg = self.generate_expr(cond, scope, storage);
                let jump_to_then_idx = self.instructions.len();
                self.emit(Opcode::Jnz, 0, cond_reg, 0, 0);
                self.next_reg = saved_reg;

                if let Some(eb) = else_branch {
                    for s in eb {
                        self.generate_stmt(s, scope, storage);
                    }
                }
                let jump_to_end_idx = self.instructions.len();
                self.emit(Opcode::Jmp, 0, 0, 0, 0);

                let then_start_idx = self.instructions.len();
                for s in then_branch {
                    self.generate_stmt(s, scope, storage);
                }
                let end_idx = self.instructions.len();

                self.patch_jump(
                    jump_to_then_idx,
                    (then_start_idx as i32) - (jump_to_then_idx as i32),
                );
                self.patch_jump(jump_to_end_idx, (end_idx as i32) - (jump_to_end_idx as i32));
            }
            Stmt::While(cond, body) => {
                let start_idx = self.instructions.len();
                let cond_reg = self.generate_expr(cond, scope, storage);

                let jump_to_body_idx = self.instructions.len();
                self.emit(Opcode::Jnz, 0, cond_reg, 0, 0);
                self.next_reg = saved_reg;

                let jump_to_end_idx = self.instructions.len();
                self.emit(Opcode::Jmp, 0, 0, 0, 0);

                let body_start_idx = self.instructions.len();
                for s in body {
                    self.generate_stmt(s, scope, storage);
                }

                let current_idx = self.instructions.len();
                self.emit(
                    Opcode::Jmp,
                    0,
                    0,
                    0,
                    (start_idx as i32) - (current_idx as i32),
                );

                let end_idx = self.instructions.len();
                self.patch_jump(
                    jump_to_body_idx,
                    (body_start_idx as i32) - (jump_to_body_idx as i32),
                );
                self.patch_jump(jump_to_end_idx, (end_idx as i32) - (jump_to_end_idx as i32));
            }
            Stmt::For {
                var,
                start,
                end,
                body,
            } => {
                let start_reg = self.generate_expr(start, scope, storage);
                let end_reg = self.generate_expr(end, scope, storage);
                let loop_reg = self.alloc_reg();
                self.emit(Opcode::Add, loop_reg, start_reg, 0, 0);
                self.next_reg = loop_reg + 1; // loop var is kept!

                let mut inner_scope = scope.clone();
                inner_scope.insert(var.clone(), loop_reg);

                let start_idx = self.instructions.len();
                
                let inner_saved = self.next_reg;
                let cond_reg = self.alloc_reg();
                self.emit(Opcode::Lt, cond_reg, loop_reg, end_reg, 0);

                let jump_to_body_idx = self.instructions.len();
                self.emit(Opcode::Jnz, 0, cond_reg, 0, 0);
                self.next_reg = inner_saved;

                let jump_to_end_idx = self.instructions.len();
                self.emit(Opcode::Jmp, 0, 0, 0, 0);

                let body_start_idx = self.instructions.len();
                for s in body {
                    self.generate_stmt(s, &mut inner_scope, storage);
                }

                let one_reg = self.alloc_reg();
                self.emit(Opcode::Load, one_reg, 0, 0, 1);
                self.emit(Opcode::Add, loop_reg, loop_reg, one_reg, 0);
                self.next_reg = inner_saved;

                let current_idx = self.instructions.len();
                self.emit(
                    Opcode::Jmp,
                    0,
                    0,
                    0,
                    (start_idx as i32) - (current_idx as i32),
                );

                let end_idx = self.instructions.len();
                self.patch_jump(
                    jump_to_body_idx,
                    (body_start_idx as i32) - (jump_to_body_idx as i32),
                );
                self.patch_jump(jump_to_end_idx, (end_idx as i32) - (jump_to_end_idx as i32));
                
                self.next_reg = saved_reg;
            }
            Stmt::Return(expr) => {
                let temp = self.alloc_reg();
                self.emit(Opcode::Pop, temp, 0, 0, 0);
                
                if let Some(e) = expr {
                    let reg = self.generate_expr(e, scope, storage);
                    self.emit(Opcode::Push, 0, reg, 0, 0);
                } else {
                    let zero = self.alloc_reg();
                    self.emit(Opcode::Load, zero, 0, 0, 0);
                    self.emit(Opcode::Push, 0, zero, 0, 0);
                }
                
                self.emit(Opcode::Push, 0, temp, 0, 0);
                self.emit(Opcode::Ret, 0, 0, 0, 0);
                self.next_reg = saved_reg;
            }
            Stmt::Emit(_name, args) => {
                for arg in args {
                    let reg = self.generate_expr(arg, scope, storage);
                    self.emit(Opcode::Log, 0, reg, 0, 0);
                }
                self.next_reg = saved_reg;
            }
            Stmt::Expr(expr) => {
                self.generate_expr(expr, scope, storage);
                self.next_reg = saved_reg;
            }
        }
    }

    fn patch_jump(&mut self, idx: usize, offset: i32) {
        if self.error.is_some() {
            return;
        }
        let inst_raw = self.instructions[idx];
        let mut inst = match Instruction::decode_any(inst_raw) {
            Ok(i) => i,
            Err(_) => {
                if self.error.is_none() {
                    self.error = Some(CompileError::CodegenError(
                        "patch_jump: failed to decode instruction".to_string(),
                    ));
                }
                return;
            }
        };
        inst.imm = offset;
        self.instructions[idx] = inst.encode();
    }

    fn generate_expr(
        &mut self,
        expr: &Expr,
        scope: &std::collections::HashMap<String, u8>,
        storage: &std::collections::HashMap<String, i32>,
    ) -> u8 {
        if self.error.is_some() {
            return 0;
        }

        match expr {
            Expr::Int(val) => {
                let v = *val;
                if v <= i32::MAX as u64 {
                    let reg = self.alloc_reg();
                    self.emit(Opcode::Load, reg, 0, 0, v as i32);
                    reg
                } else {
                    let chunks = [
                        ((v >> 60) & 0xF) as i32,
                        ((v >> 30) & 0x3FFFFFFF) as i32,
                        (v & 0x3FFFFFFF) as i32,
                    ];
                    let reg = self.alloc_reg();
                    let shift_reg = self.alloc_reg();
                    self.emit(Opcode::Load, shift_reg, 0, 0, 1073741824); // 2^30
                    let temp_reg = self.alloc_reg();
                    
                    let mut started = false;
                    for i in 0..3 {
                        if chunks[i] > 0 || started {
                            if started {
                                self.emit(Opcode::Mul, reg, reg, shift_reg, 0);
                                if chunks[i] > 0 {
                                    self.emit(Opcode::Load, temp_reg, 0, 0, chunks[i]);
                                    self.emit(Opcode::Add, reg, reg, temp_reg, 0);
                                }
                            } else {
                                started = true;
                                self.emit(Opcode::Load, reg, 0, 0, chunks[i]);
                            }
                        }
                    }
                    reg
                }
            }
            Expr::Ident(name) => match scope.get(name) {
                Some(r) => *r,
                None => {
                    if self.error.is_none() {
                        self.error = Some(CompileError::CodegenError(format!(
                            "Undefined variable in codegen: {}",
                            name
                        )));
                    }
                    0
                }
            },
            Expr::StorageRead(name) => {
                let reg = self.alloc_reg();
                let slot = match storage.get(name) {
                    Some(s) => *s,
                    None => {
                        if self.error.is_none() {
                            self.error = Some(CompileError::CodegenError(format!(
                                "Unknown storage variable in codegen: {}",
                                name
                            )));
                        }
                        return 0;
                    }
                };
                self.emit(Opcode::SRead, reg, 0, 0, slot);
                reg
            }
            Expr::MappingRead(name, key) => {
                let base_slot = match storage.get(name) {
                    Some(s) => *s,
                    None => {
                        if self.error.is_none() {
                            self.error = Some(CompileError::CodegenError(format!(
                                "Unknown mapping in codegen: {}",
                                name
                            )));
                        }
                        return 0;
                    }
                };
                let key_reg = self.generate_expr(key, scope, storage);

                let base_reg = self.alloc_reg();
                self.emit(Opcode::Load, base_reg, 0, 0, base_slot);

                let target_slot_reg = self.alloc_reg();
                self.emit(Opcode::Poseidon, target_slot_reg, base_reg, key_reg, 0);

                let res_reg = self.alloc_reg();
                self.emit(Opcode::SRead, res_reg, 0, target_slot_reg, -1);
                res_reg
            }
            Expr::StructLiteral(_name, fields) => {
                let saved_next_reg = self.next_reg;
                let ptr_reg = self.alloc_reg();
                self.emit(Opcode::Add, ptr_reg, 31, 0, 0); // copy heap ptr to ptr_reg
                
                let mut field_vals = Vec::new();
                for (_, val) in fields {
                    field_vals.push(self.generate_expr(val, scope, storage));
                }
                
                for (i, val_reg) in field_vals.into_iter().enumerate() {
                    self.emit(Opcode::Store, 0, ptr_reg, val_reg, (i * 8) as i32);
                }
                
                let size_reg = self.alloc_reg();
                self.emit(Opcode::Load, size_reg, 0, 0, (fields.len() * 8) as i32);
                self.emit(Opcode::Add, 31, 31, size_reg, 0); // bump heap pointer
                
                self.next_reg = saved_next_reg; 
                let res_reg = self.alloc_reg();
                self.emit(Opcode::Add, res_reg, ptr_reg, 0, 0); // return pointer
                res_reg
            }
            Expr::FieldAccess(base, field) => {
                let base_reg = self.generate_expr(base, scope, storage);
                let res_reg = self.alloc_reg();
                
                let mut offset = 0;
                for fields in self.struct_layouts.values() {
                    if let Some(idx) = fields.iter().position(|f| f == field) {
                        offset = idx * 8;
                        break;
                    }
                }
                self.emit(Opcode::Load, res_reg, base_reg, 0, offset as i32);
                res_reg
            }
            Expr::Binary(left, op, right) => {
                let saved1 = self.next_reg;
                let l_reg = self.generate_expr(left, scope, storage);
                let saved2 = self.next_reg;
                let r_reg = self.generate_expr(right, scope, storage);
                
                let res_reg = if l_reg >= saved1 {
                    l_reg
                } else if r_reg >= saved2 {
                    r_reg
                } else {
                    self.alloc_reg()
                };

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
                self.next_reg = std::cmp::max(res_reg + 1, saved1);
                res_reg
            }
            Expr::Call(name, args) => {
                if name == "poseidon" {
                    let saved1 = self.next_reg;
                    let r1 = self.generate_expr(&args[0], scope, storage);
                    let saved2 = self.next_reg;
                    let r2 = self.generate_expr(&args[1], scope, storage);
                    
                    let res = if r1 >= saved1 {
                        r1
                    } else if r2 >= saved2 {
                        r2
                    } else {
                        self.alloc_reg()
                    };
                    
                    self.emit(Opcode::Poseidon, res, r1, r2, 0);
                    self.next_reg = std::cmp::max(res + 1, saved1);
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
                    let saved_next_reg = self.next_reg;
                    for r in 1..saved_next_reg {
                        self.emit(Opcode::Push, 0, r, 0, 0);
                    }
                    
                    let mut arg_regs = Vec::new();
                    for arg in args {
                        arg_regs.push(self.generate_expr(arg, scope, storage));
                    }
                    for arg_reg in arg_regs {
                        self.emit(Opcode::Push, 0, arg_reg, 0, 0);
                    }
                    
                    let call_idx = self.instructions.len();
                    self.emit(Opcode::Call, 0, 0, 0, 0);
                    self.unpatched_calls.push((call_idx, name.clone()));
                    
                    self.next_reg = saved_next_reg;
                    let res_reg = self.alloc_reg();
                    self.emit(Opcode::Pop, res_reg, 0, 0, 0);
                    
                    for r in (1..saved_next_reg).rev() {
                        self.emit(Opcode::Pop, r, 0, 0, 0);
                    }
                    
                    res_reg
                }
            }
        }
    }

    fn alloc_reg(&mut self) -> u8 {
        if self.next_reg >= 31 {
            if self.error.is_none() {
                self.error = Some(CompileError::RegisterExhausted);
            }
            return 30; // 31 is reserved for heap ptr
        }
        let r = self.next_reg;
        self.next_reg += 1;
        r
    }

    fn emit(&mut self, opcode: Opcode, rd: u8, rs1: u8, rs2: u8, imm: i32) {
        if opcode.is_experimental() {
            #[cfg(not(feature = "experimental"))]
            {
                if self.error.is_none() {
                    self.error = Some(CompileError::ExperimentalOpcodeDisabled(format!(
                        "Opcode {:?} is experimental and disabled in production",
                        opcode
                    )));
                }
                return;
            }

            #[cfg(feature = "experimental")]
            if self.profile == IsaProfile::Production {
                if self.error.is_none() {
                    self.error = Some(CompileError::ExperimentalOpcodeDisabled(format!(
                        "Opcode {:?} is experimental and disabled in production",
                        opcode
                    )));
                }
                return;
            }
        }

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
