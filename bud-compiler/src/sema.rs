use crate::ast::*;
use std::collections::HashSet;

pub struct SemanticAnalyzer {
    symbols: HashSet<String>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            symbols: HashSet::new(),
        }
    }

    pub fn analyze(&mut self, contract: &Contract) {
        for func in &contract.functions {
            self.analyze_function(func);
        }
    }

    fn analyze_function(&mut self, func: &Function) {
        let mut local_symbols = HashSet::new();
        for param in &func.params {
            local_symbols.insert(param.name.clone());
        }
        for stmt in &func.body {
            match stmt {
                Stmt::Let(name, expr) => {
                    self.analyze_expr(expr, &local_symbols);
                    local_symbols.insert(name.clone());
                }
                Stmt::Constrain(expr) => {
                    self.analyze_expr(expr, &local_symbols);
                }
                Stmt::Assign(name, expr) => {
                    if !local_symbols.contains(name) {
                        panic!("Undefined variable: {}", name);
                    }
                    self.analyze_expr(expr, &local_symbols);
                }
                Stmt::StorageWrite(_, expr) => {
                    self.analyze_expr(expr, &local_symbols);
                }
                Stmt::If(cond, then_branch, else_branch) => {
                    self.analyze_expr(cond, &local_symbols);
                    for s in then_branch {
                        self.analyze_stmt_recursive(s, &mut local_symbols);
                    }
                    if let Some(eb) = else_branch {
                        for s in eb {
                            self.analyze_stmt_recursive(s, &mut local_symbols);
                        }
                    }
                }
                Stmt::MappingWrite(_, key, val) => {
                    self.analyze_expr(key, &local_symbols);
                    self.analyze_expr(val, &local_symbols);
                }
                Stmt::Emit(_, args) => {
                    for arg in args {
                        self.analyze_expr(arg, &local_symbols);
                    }
                }
                Stmt::While(cond, body) => {
                    self.analyze_expr(cond, &local_symbols);
                    for s in body {
                        self.analyze_stmt_recursive(s, &mut local_symbols);
                    }
                }
                Stmt::Return(expr) => {
                    if let Some(e) = expr {
                        self.analyze_expr(e, &local_symbols);
                    }
                }
                Stmt::Expr(expr) => {
                    self.analyze_expr(expr, &local_symbols);
                }
            }
        }
    }

    fn analyze_stmt_recursive(&self, stmt: &Stmt, symbols: &mut HashSet<String>) {
        match stmt {
            Stmt::Let(name, expr) => {
                self.analyze_expr(expr, symbols);
                symbols.insert(name.clone());
            }
            Stmt::While(cond, body) => {
                self.analyze_expr(cond, symbols);
                for s in body {
                    self.analyze_stmt_recursive(s, symbols);
                }
            }
            Stmt::If(cond, then_branch, else_branch) => {
                self.analyze_expr(cond, symbols);
                for s in then_branch {
                    self.analyze_stmt_recursive(s, symbols);
                }
                if let Some(eb) = else_branch {
                    for s in eb {
                        self.analyze_stmt_recursive(s, symbols);
                    }
                }
            }
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.analyze_expr(e, symbols);
                }
            }
            _ => {}
        }
    }

    fn analyze_expr(&self, expr: &Expr, locals: &HashSet<String>) {
        match expr {
            Expr::Int(_) => {}
            Expr::Ident(name) => {
                if !locals.contains(name) {
                    panic!("Undefined identifier: {}", name);
                }
            }
            Expr::StorageRead(_) => {}
            Expr::MappingRead(_, key) => {
                self.analyze_expr(key, locals);
            }
            Expr::Call(name, args) => {
                if name == "verify_merkle_proof" {
                    for arg in args {
                        self.analyze_expr(arg, locals);
                    }
                } else {
                    for arg in args {
                        self.analyze_expr(arg, locals);
                    }
                }
            }
            Expr::Binary(left, _, right) => {
                self.analyze_expr(left, locals);
                self.analyze_expr(right, locals);
            }
        }
    }
}
