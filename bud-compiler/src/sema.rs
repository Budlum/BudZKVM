use crate::ast::*;
use crate::CompileError;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    U64,
    Bool,
    Field,
    Struct(String),
    Void,
    Unknown,
}

impl Type {
    fn from_str(s: &str) -> Result<Type, String> {
        match s {
            "u64" => Ok(Type::U64),
            "bool" => Ok(Type::Bool),
            "field" => Ok(Type::Field),
            _ => Ok(Type::Struct(s.to_string())), // Assume it's a struct type
        }
    }
}

pub struct SemanticAnalyzer {
    pub structs: HashMap<String, HashMap<String, Type>>,
    pub functions: HashMap<String, (Vec<Type>, Type)>,
    pub current_func_ret: Type,
}

impl Default for SemanticAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            structs: HashMap::new(),
            functions: HashMap::new(),
            current_func_ret: Type::Void,
        }
    }

    pub fn analyze(&mut self, contract: &Contract) -> Result<(), CompileError> {
        let mut errors = Vec::new();

        // 1. Register structs
        for s in &contract.structs {
            let mut fields = HashMap::new();
            for f in &s.fields {
                let ty = Type::from_str(&f.ty).map_err(|e| CompileError::SemanticError(e))?;
                fields.insert(f.name.clone(), ty);
            }
            self.structs.insert(s.name.clone(), fields);
        }

        // 2. Register functions
        for f in &contract.functions {
            let mut params = Vec::new();
            for p in &f.params {
                let ty = Type::from_str(&p.ty).unwrap_or(Type::Unknown);
                params.push(ty);
            }
            let ret_ty = if let Some(r) = &f.return_type {
                Type::from_str(r).unwrap_or(Type::Unknown)
            } else {
                Type::Void
            };
            self.functions.insert(f.name.clone(), (params, ret_ty));
        }

        // 3. Builtins
        self.functions.insert("poseidon".to_string(), (vec![Type::U64, Type::U64], Type::U64));
        self.functions.insert("verify_merkle_proof".to_string(), (vec![Type::U64, Type::U64, Type::U64], Type::U64));
        self.functions.insert("msg::sender".to_string(), (vec![], Type::U64));
        self.functions.insert("msg::nonce".to_string(), (vec![], Type::U64));
        self.functions.insert("block::number".to_string(), (vec![], Type::U64));

        for func in &contract.functions {
            self.analyze_function(func, &mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.remove(0))
        }
    }

    fn analyze_function(&mut self, func: &Function, errors: &mut Vec<CompileError>) {
        let mut env = HashMap::new();
        for param in &func.params {
            let ty = Type::from_str(&param.ty).unwrap_or(Type::Unknown);
            env.insert(param.name.clone(), ty);
        }
        self.current_func_ret = if let Some(r) = &func.return_type {
            Type::from_str(r).unwrap_or(Type::Unknown)
        } else {
            Type::Void
        };

        for stmt in &func.body {
            self.analyze_stmt(stmt, &mut env, errors);
        }
    }

    fn analyze_stmt(
        &mut self,
        stmt: &Stmt,
        env: &mut HashMap<String, Type>,
        errors: &mut Vec<CompileError>,
    ) {
        match stmt {
            Stmt::Let(name, expr) => {
                let ty = self.analyze_expr(expr, env, errors);
                env.insert(name.clone(), ty);
            }
            Stmt::Constrain(expr) => {
                self.analyze_expr(expr, env, errors);
            }
            Stmt::Assign(name, expr) => {
                if let Some(expected_ty) = env.get(name).cloned() {
                    let ty = self.analyze_expr(expr, env, errors);
                    if ty != expected_ty && ty != Type::Unknown && expected_ty != Type::Unknown {
                        errors.push(CompileError::SemanticError(format!(
                            "Type mismatch in assign: expected {:?}, got {:?}", expected_ty, ty
                        )));
                    }
                } else {
                    errors.push(CompileError::SemanticError(format!("Undefined variable: {}", name)));
                    self.analyze_expr(expr, env, errors);
                }
            }
            Stmt::StorageWrite(_, expr) => {
                self.analyze_expr(expr, env, errors);
            }
            Stmt::MappingWrite(_, key, val) => {
                self.analyze_expr(key, env, errors);
                self.analyze_expr(val, env, errors);
            }
            Stmt::If(cond, then_branch, else_branch) => {
                self.analyze_expr(cond, env, errors);
                for s in then_branch {
                    self.analyze_stmt(s, env, errors);
                }
                if let Some(eb) = else_branch {
                    for s in eb {
                        self.analyze_stmt(s, env, errors);
                    }
                }
            }
            Stmt::While(cond, body) => {
                self.analyze_expr(cond, env, errors);
                for s in body {
                    self.analyze_stmt(s, env, errors);
                }
            }
            Stmt::For { var, start, end, body } => {
                self.analyze_expr(start, env, errors);
                self.analyze_expr(end, env, errors);
                let mut inner_env = env.clone();
                inner_env.insert(var.clone(), Type::U64);
                for s in body {
                    self.analyze_stmt(s, &mut inner_env, errors);
                }
            }
            Stmt::Return(expr) => {
                let ret_ty = if let Some(e) = expr {
                    self.analyze_expr(e, env, errors)
                } else {
                    Type::Void
                };
                if ret_ty != self.current_func_ret && ret_ty != Type::Unknown && self.current_func_ret != Type::Unknown {
                    errors.push(CompileError::SemanticError(format!(
                        "Type mismatch in return: expected {:?}, got {:?}", self.current_func_ret, ret_ty
                    )));
                }
            }
            Stmt::Emit(_, args) => {
                for arg in args {
                    self.analyze_expr(arg, env, errors);
                }
            }
            Stmt::Expr(expr) => {
                self.analyze_expr(expr, env, errors);
            }
        }
    }

    fn analyze_expr(
        &mut self,
        expr: &Expr,
        env: &HashMap<String, Type>,
        errors: &mut Vec<CompileError>,
    ) -> Type {
        match expr {
            Expr::Int(_) => Type::U64,
            Expr::Ident(name) => {
                if let Some(ty) = env.get(name) {
                    ty.clone()
                } else {
                    errors.push(CompileError::SemanticError(format!("Undefined identifier: {}", name)));
                    Type::Unknown
                }
            }
            Expr::StorageRead(_) => Type::U64,
            Expr::MappingRead(_, key) => {
                self.analyze_expr(key, env, errors);
                Type::U64
            }
            Expr::FieldAccess(base, field) => {
                let base_ty = self.analyze_expr(base, env, errors);
                if let Type::Struct(sname) = base_ty {
                    if let Some(fields) = self.structs.get(&sname) {
                        if let Some(fty) = fields.get(field) {
                            return fty.clone();
                        } else {
                            errors.push(CompileError::SemanticError(format!("Struct {} has no field {}", sname, field)));
                        }
                    }
                } else if base_ty != Type::Unknown {
                    errors.push(CompileError::SemanticError("Field access on non-struct".to_string()));
                }
                Type::Unknown
            }
            Expr::StructLiteral(name, fields) => {
                if let Some(sfields) = self.structs.get(name).cloned() {
                    for (fname, val) in fields {
                        let ty = self.analyze_expr(val, env, errors);
                        if let Some(expected_ty) = sfields.get(fname) {
                            if ty != *expected_ty && ty != Type::Unknown {
                                errors.push(CompileError::SemanticError(format!("Field {} type mismatch", fname)));
                            }
                        } else {
                            errors.push(CompileError::SemanticError(format!("Unknown field {}", fname)));
                        }
                    }
                    Type::Struct(name.clone())
                } else {
                    errors.push(CompileError::SemanticError(format!("Undefined struct: {}", name)));
                    Type::Unknown
                }
            }
            Expr::Call(name, args) => {
                let mut arg_types = Vec::new();
                for arg in args {
                    arg_types.push(self.analyze_expr(arg, env, errors));
                }
                if let Some((params, ret_ty)) = self.functions.get(name) {
                    if params.len() != args.len() {
                        errors.push(CompileError::SemanticError(format!("Function {} expects {} args, got {}", name, params.len(), args.len())));
                    } else {
                        for (i, (exp, act)) in params.iter().zip(arg_types.iter()).enumerate() {
                            if exp != act && act != &Type::Unknown && exp != &Type::Unknown {
                                errors.push(CompileError::SemanticError(format!("Arg {} type mismatch in {}", i, name)));
                            }
                        }
                    }
                    ret_ty.clone()
                } else {
                    errors.push(CompileError::SemanticError(format!("Undefined function: {}", name)));
                    Type::Unknown
                }
            }
            Expr::Binary(left, _, right) => {
                let l_ty = self.analyze_expr(left, env, errors);
                let r_ty = self.analyze_expr(right, env, errors);
                if l_ty != r_ty && l_ty != Type::Unknown && r_ty != Type::Unknown {
                    errors.push(CompileError::SemanticError("Type mismatch in binary expression".to_string()));
                }
                l_ty // All binary ops return same type (or u64 for comparisons, which is currently our only type)
            }
        }
    }
}
