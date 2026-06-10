import re

with open("bud-compiler/src/codegen.rs", "r") as f:
    content = f.read()

# 1. Update struct Codegen
struct_orig = """pub struct Codegen {
    instructions: Vec<u64>,
    next_reg: u8,
    profile: IsaProfile,
    error: Option<CompileError>,
}"""
struct_new = """pub struct Codegen {
    instructions: Vec<u64>,
    next_reg: u8,
    profile: IsaProfile,
    error: Option<CompileError>,
    unpatched_calls: Vec<(usize, String)>,
}"""
content = content.replace(struct_orig, struct_new)

# 2. Update new()
new_orig = """    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            next_reg: 1,
            profile: IsaProfile::Production,
            error: None,
        }
    }"""
new_new = """    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            next_reg: 1,
            profile: IsaProfile::Production,
            error: None,
            unpatched_calls: Vec::new(),
        }
    }"""
content = content.replace(new_orig, new_new)

# 3. Update new_with_profile()
new_with_prof_orig = """    pub fn new_with_profile(profile: IsaProfile) -> Self {
        Self {
            instructions: Vec::new(),
            next_reg: 1,
            profile,
            error: None,
        }
    }"""
new_with_prof_new = """    pub fn new_with_profile(profile: IsaProfile) -> Self {
        Self {
            instructions: Vec::new(),
            next_reg: 1,
            profile,
            error: None,
            unpatched_calls: Vec::new(),
        }
    }"""
content = content.replace(new_with_prof_orig, new_with_prof_new)

# 4. Update generate()
gen_orig = """    pub fn generate(&mut self, contract: &Contract) -> Result<Vec<u64>, CompileError> {
        for func in &contract.functions {
            self.generate_function(func, contract);
        }
        self.emit(Opcode::Halt, 0, 0, 0, 0);

        if let Some(err) = self.error.take() {"""
gen_new = """    pub fn generate(&mut self, contract: &Contract) -> Result<Vec<u64>, CompileError> {
        let jump_to_main_idx = self.instructions.len();
        self.emit(Opcode::Call, 0, 0, 0, 0);
        self.emit(Opcode::Halt, 0, 0, 0, 0);

        let mut func_offsets = std::collections::HashMap::new();

        for func in &contract.functions {
            func_offsets.insert(func.name.clone(), self.instructions.len());
            self.generate_function(func, contract);
        }

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

        if let Some(err) = self.error.take() {"""
content = content.replace(gen_orig, gen_new)

# 5. Update generate_function()
gen_func_orig = """    fn generate_function(&mut self, func: &Function, contract: &Contract) {
        if self.error.is_some() {
            return;
        }

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
            self.generate_stmt(stmt, &mut scope, &storage_map);
        }
    }"""
gen_func_new = """    fn generate_function(&mut self, func: &Function, contract: &Contract) {
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
    }"""
content = content.replace(gen_func_orig, gen_func_new)

# 6. Update Stmt::Return
ret_orig = """            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    let reg = self.generate_expr(e, scope, storage);
                    self.emit(Opcode::Load, 1, reg, 0, 0);
                }
                self.emit(Opcode::Halt, 0, 0, 0, 0);
                self.next_reg = saved_reg;
            }"""
ret_new = """            Stmt::Return(expr) => {
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
            }"""
content = content.replace(ret_orig, ret_new)

# 7. Update Expr::Call user function call fallback
call_orig = """                    let res = self.alloc_reg();
                    self.emit(Opcode::VerifyMerkle, res, r_root, r_leaf, r_path as i32);
                    res
                } else {
                    0
                }
            }
        }
    }"""
call_new = """                    let res = self.alloc_reg();
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
    }"""
content = content.replace(call_orig, call_new)

with open("bud-compiler/src/codegen.rs", "w") as f:
    f.write(content)
