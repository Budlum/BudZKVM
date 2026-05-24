use crate::ast::*;
use crate::CompileError;
use std::collections::HashSet;

pub struct SemanticAnalyzer {}

impl Default for SemanticAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {}
    }

    pub fn analyze(&mut self, contract: &Contract) -> Result<(), CompileError> {
        let mut errors = Vec::new();
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
        let mut local_symbols = HashSet::new();
        for param in &func.params {
            local_symbols.insert(param.name.clone());
        }
        for stmt in &func.body {
            self.analyze_stmt(stmt, &mut local_symbols, errors);
        }
    }

    fn analyze_stmt(
        &mut self,
        stmt: &Stmt,
        symbols: &mut HashSet<String>,
        errors: &mut Vec<CompileError>,
    ) {
        match stmt {
            Stmt::Let(name, expr) => {
                self.analyze_expr(expr, symbols, errors);
                symbols.insert(name.clone());
            }
            Stmt::Constrain(expr) => {
                self.analyze_expr(expr, symbols, errors);
            }
            Stmt::Assign(name, expr) => {
                if !symbols.contains(name) {
                    errors.push(CompileError::SemanticError(format!(
                        "Undefined variable: {}",
                        name
                    )));
                }
                self.analyze_expr(expr, symbols, errors);
            }
            Stmt::StorageWrite(_, expr) => {
                self.analyze_expr(expr, symbols, errors);
            }
            Stmt::MappingWrite(_, key, val) => {
                self.analyze_expr(key, symbols, errors);
                self.analyze_expr(val, symbols, errors);
            }
            Stmt::If(cond, then_branch, else_branch) => {
                self.analyze_expr(cond, symbols, errors);
                for s in then_branch {
                    self.analyze_stmt(s, symbols, errors);
                }
                if let Some(eb) = else_branch {
                    for s in eb {
                        self.analyze_stmt(s, symbols, errors);
                    }
                }
            }
            Stmt::While(cond, body) => {
                self.analyze_expr(cond, symbols, errors);
                for s in body {
                    self.analyze_stmt(s, symbols, errors);
                }
            }
            Stmt::For {
                var,
                start,
                end,
                body,
            } => {
                self.analyze_expr(start, symbols, errors);
                self.analyze_expr(end, symbols, errors);
                let mut inner_symbols = symbols.clone();
                inner_symbols.insert(var.clone());
                for s in body {
                    self.analyze_stmt(s, &mut inner_symbols, errors);
                }
            }
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.analyze_expr(e, symbols, errors);
                }
            }
            Stmt::Emit(_, args) => {
                for arg in args {
                    self.analyze_expr(arg, symbols, errors);
                }
            }
            Stmt::Expr(expr) => {
                self.analyze_expr(expr, symbols, errors);
            }
        }
    }

    fn analyze_expr(
        &mut self,
        expr: &Expr,
        locals: &HashSet<String>,
        errors: &mut Vec<CompileError>,
    ) {
        match expr {
            Expr::Int(_) => {}
            Expr::Ident(name) => {
                if !locals.contains(name) {
                    errors.push(CompileError::SemanticError(format!(
                        "Undefined identifier: {}",
                        name
                    )));
                }
            }
            Expr::StorageRead(_) => {}
            Expr::MappingRead(_, key) => {
                self.analyze_expr(key, locals, errors);
            }
            Expr::Call(_name, args) => {
                for arg in args {
                    self.analyze_expr(arg, locals, errors);
                }
            }
            Expr::Binary(left, _, right) => {
                self.analyze_expr(left, locals, errors);
                self.analyze_expr(right, locals, errors);
            }
        }
    }
}
