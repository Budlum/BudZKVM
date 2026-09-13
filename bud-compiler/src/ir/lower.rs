//! AST → BudIR lowering.
//!
//! Bu modül mevcut `codegen.rs`'den tamamen bağımsızdır: fiziksel register,
//! PC offset veya ISA opcode bilgisi içermez.
//!
//! ## Desteklenen subset (ilk sürüm)
//! - İfadeler: integer literal, identifier, aritmetik, karşılaştırma,
//!   fonksiyon çağrısı, storage read, context built-in'ler, poseidon(…)
//! - İfadeler (henüz desteklenmiyor): struct literal, mapping read/write
//! - Deyimler: let, assign, constrain, if/else, while, for, return,
//!   emit, storage write, expression statement
//!
//! ## SSA kararı
//! Değişebilir değişkenler (BudL `let` + atama) `LocalId` slotları üzerinden
//! modellenir. Her hesaplamanın sonucu benzersiz bir `ValueId` üretir; ancak
//! mutable değişkene birden fazla kez yazılabilir. Bu pre-SSA / "mem2reg öncesi"
//! stildir — ileride bir `mem2reg` geçişi ile tam SSA'ya yükseltilebilir.
//! Phi node gerektiren branch merge durumları için `ReadLocal` daima slottaki
//! en son yazılan değeri okur; CFG üzerinde dominance analizi yapılmaz.

use std::collections::HashMap;

use crate::ast::{BinOp, Contract, Expr, Stmt};
use crate::sema::{self, SemanticAnalyzer};

use super::{
    BasicBlock, BlockId, ContextKind, FunctionId, InstrNode, Instruction, IrFunction, IrProgram,
    IrType, LocalId, Terminator, ValueId,
};

// ─── Hata tipi ────────────────────────────────────────────────────────────

/// AST → BudIR dönüşümü sırasında oluşabilecek hatalar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    /// Tipi çözümlenemeyen sema::Type::Unknown ya da tanınmayan tür adı.
    UnresolvedType(String),
    /// Tanımsız değişken adı.
    UndefinedVariable(String),
    /// Tanımsız fonksiyon adı.
    UndefinedFunction(String),
    /// Bu sürümde desteklenmeyen AST düğümü.
    UnsupportedNode(String),
    /// Tip uyumsuzluğu (sema zaten yakalamalı, ama savunmacı kontrol).
    TypeMismatch { expected: IrType, got: IrType },
    /// Lowerer içindeki tutarsızlık (bug).
    InternalError(String),
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoweringError::UnresolvedType(t) => write!(f, "unresolved type: {t}"),
            LoweringError::UndefinedVariable(v) => write!(f, "undefined variable: {v}"),
            LoweringError::UndefinedFunction(fn_) => write!(f, "undefined function: {fn_}"),
            LoweringError::UnsupportedNode(n) => write!(f, "unsupported AST node: {n}"),
            LoweringError::TypeMismatch { expected, got } => {
                write!(f, "type mismatch: expected {expected:?}, got {got:?}")
            }
            LoweringError::InternalError(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for LoweringError {}

// ─── Yardımcı dönüşümler ──────────────────────────────────────────────────

/// `sema::Type` → `IrType`. Unknown lowering hatasına dönüşür.
fn from_sema_type(ty: &sema::Type) -> Result<IrType, LoweringError> {
    match ty {
        sema::Type::U64 => Ok(IrType::U64),
        sema::Type::Bool => Ok(IrType::Bool),
        sema::Type::Field => Ok(IrType::Field),
        sema::Type::Struct(n) => Ok(IrType::Struct(n.clone())),
        sema::Type::Void => Ok(IrType::Void),
        sema::Type::Unknown => Err(LoweringError::UnresolvedType("Unknown".into())),
    }
}

/// String tür adı → `IrType`.
fn parse_ir_type(s: &str) -> Result<IrType, LoweringError> {
    match s {
        "u64" => Ok(IrType::U64),
        "bool" => Ok(IrType::Bool),
        "field" => Ok(IrType::Field),
        "void" | "" => Ok(IrType::Void),
        other => Ok(IrType::Struct(other.to_string())),
    }
}

// ─── Genel giriş noktası ──────────────────────────────────────────────────

/// Bir contract'ı [`IrProgram`]'a dönüştürür.
///
/// Mevcut production codegen (`codegen.rs`) değiştirilmez; bu fonksiyon
/// paralel/alternatif bir lowering yolu sunar.
pub fn lower_contract(
    contract: &Contract,
    sema: &SemanticAnalyzer,
) -> Result<IrProgram, LoweringError> {
    let mut lowerer = Lowerer::new(contract, sema);
    lowerer.lower(contract)
}

// ─── Lowerer ──────────────────────────────────────────────────────────────

struct Lowerer<'a> {
    sema: &'a SemanticAnalyzer,
    /// Storage alan adı → slot indeksi (0-tabanlı).
    storage_slots: HashMap<String, i32>,
    /// Struct adı → sıralı alan adları listesi (byte-offset hesabı için).
    struct_field_order: HashMap<String, Vec<String>>,
    /// Fonksiyon adı → FunctionId (birinci geçiş).
    function_ids: HashMap<String, FunctionId>,
}

impl<'a> Lowerer<'a> {
    fn new(contract: &Contract, sema: &'a SemanticAnalyzer) -> Self {
        let storage_slots = contract
            .storage
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.clone(), i as i32))
            .collect();

        let struct_field_order = contract
            .structs
            .iter()
            .map(|s| {
                let fields = s.fields.iter().map(|f| f.name.clone()).collect();
                (s.name.clone(), fields)
            })
            .collect();

        Lowerer {
            sema,
            storage_slots,
            struct_field_order,
            function_ids: HashMap::new(),
        }
    }

    fn lower(&mut self, contract: &Contract) -> Result<IrProgram, LoweringError> {
        let mut program = IrProgram::default();

        // 1. Geçiş: tüm fonksiyonlara FunctionId ata.
        for (i, func) in contract.functions.iter().enumerate() {
            let fid = FunctionId(i as u32);
            self.function_ids.insert(func.name.clone(), fid);
            program.function_names.insert(func.name.clone(), fid);
        }

        // 2. Geçiş: her fonksiyonu lower et.
        for (i, func) in contract.functions.iter().enumerate() {
            let ir_fn = self.lower_function(FunctionId(i as u32), func)?;
            program.functions.push(ir_fn);
        }

        Ok(program)
    }

    fn lower_function(
        &mut self,
        fid: FunctionId,
        func: &crate::ast::Function,
    ) -> Result<IrFunction, LoweringError> {
        let ret_ty = match &func.return_type {
            Some(t) => parse_ir_type(t)?,
            None => IrType::Void,
        };

        let mut ctx = FnCtx::new(fid, ret_ty.clone());

        // Parametreler: her biri bir ValueId alır, arka planda bir LocalId ile desteklenir.
        let mut params = Vec::new();
        for param in &func.params {
            let ty = parse_ir_type(&param.ty)?;
            let val = ctx.fresh_value(); // parametre değeri
            params.push((val, ty.clone()));
            let local = ctx.fresh_local(ty.clone());
            ctx.define_local(&param.name, local, ty);
            // Slotu parametre değeri ile başlat.
            ctx.push_instr(Instruction::WriteLocal { local, value: val });
        }

        // Gövdeyi lower et.
        for stmt in &func.body {
            self.lower_stmt(stmt, &mut ctx)?;
        }

        // Açık return yoksa void return ekle.
        if !ctx.current_block_terminated() {
            ctx.set_terminator(Terminator::Return(None));
        }

        // Blokları deterministik sırayla topla.
        let mut blocks = ctx.blocks;
        blocks.sort_by_key(|b| b.id);

        Ok(IrFunction {
            id: fid,
            name: func.name.clone(),
            params,
            ret_ty,
            blocks,
            locals: ctx.locals,
        })
    }

    // ── Deyim lowering ────────────────────────────────────────────────────

    fn lower_stmt(&mut self, stmt: &Stmt, ctx: &mut FnCtx) -> Result<(), LoweringError> {
        // Terminate edilmiş bloğa kod üretme.
        if ctx.current_block_terminated() {
            return Ok(());
        }

        match stmt {
            Stmt::Let(name, expr) => {
                let (val, ty) = self.lower_expr_value(expr, ctx)?;
                let local = ctx.fresh_local(ty.clone());
                ctx.define_local(name, local, ty);
                ctx.push_instr(Instruction::WriteLocal { local, value: val });
            }

            Stmt::Assign(name, expr) => {
                let (val, _) = self.lower_expr_value(expr, ctx)?;
                let (local, _) = ctx
                    .lookup_local(name)
                    .ok_or_else(|| LoweringError::UndefinedVariable(name.clone()))?;
                ctx.push_instr(Instruction::WriteLocal { local, value: val });
            }

            Stmt::Constrain(expr) => {
                let (val, _) = self.lower_expr_value(expr, ctx)?;
                ctx.push_instr(Instruction::Assert { condition: val });
            }

            Stmt::StorageWrite(name, expr) => {
                let slot = *self
                    .storage_slots
                    .get(name)
                    .ok_or_else(|| LoweringError::UndefinedVariable(name.clone()))?;
                let (val, _) = self.lower_expr_value(expr, ctx)?;
                ctx.push_instr(Instruction::StateWrite { slot, value: val });
            }

            Stmt::If(cond, then_body, else_body) => {
                let (cond_val, _) = self.lower_expr_value(cond, ctx)?;
                let then_block = ctx.fresh_block();
                let else_block = ctx.fresh_block();
                let merge_block = ctx.fresh_block();

                ctx.set_terminator(Terminator::Branch {
                    condition: cond_val,
                    then_block,
                    else_block,
                });

                // then dalı
                ctx.switch_to(then_block);
                ctx.push_scope();
                for s in then_body {
                    self.lower_stmt(s, ctx)?;
                }
                ctx.pop_scope();
                if !ctx.current_block_terminated() {
                    ctx.set_terminator(Terminator::Jump(merge_block));
                }

                // else dalı
                ctx.switch_to(else_block);
                ctx.push_scope();
                if let Some(eb) = else_body {
                    for s in eb {
                        self.lower_stmt(s, ctx)?;
                    }
                }
                ctx.pop_scope();
                if !ctx.current_block_terminated() {
                    ctx.set_terminator(Terminator::Jump(merge_block));
                }

                ctx.switch_to(merge_block);
            }

            Stmt::While(cond, body) => {
                let header_block = ctx.fresh_block();
                let body_block = ctx.fresh_block();
                let exit_block = ctx.fresh_block();

                ctx.set_terminator(Terminator::Jump(header_block));

                // header: koşul değerlendirmesi
                ctx.switch_to(header_block);
                let (cond_val, _) = self.lower_expr_value(cond, ctx)?;
                ctx.set_terminator(Terminator::Branch {
                    condition: cond_val,
                    then_block: body_block,
                    else_block: exit_block,
                });

                // gövde
                ctx.switch_to(body_block);
                ctx.push_scope();
                for s in body {
                    self.lower_stmt(s, ctx)?;
                }
                ctx.pop_scope();
                if !ctx.current_block_terminated() {
                    ctx.set_terminator(Terminator::Jump(header_block));
                }

                ctx.switch_to(exit_block);
            }

            Stmt::For {
                var,
                start,
                end,
                body,
            } => {
                let (start_val, _) = self.lower_expr_value(start, ctx)?;
                let (end_val, _) = self.lower_expr_value(end, ctx)?;

                // Döngü değişkeni için local slot.
                let loop_local = ctx.fresh_local(IrType::U64);
                ctx.push_instr(Instruction::WriteLocal {
                    local: loop_local,
                    value: start_val,
                });

                // end değerini de bir slotta tut (header block'ta referans gerekebilir).
                let end_local = ctx.fresh_local(IrType::U64);
                ctx.push_instr(Instruction::WriteLocal {
                    local: end_local,
                    value: end_val,
                });

                let header_block = ctx.fresh_block();
                let body_block = ctx.fresh_block();
                let exit_block = ctx.fresh_block();

                ctx.set_terminator(Terminator::Jump(header_block));

                // header: i < end
                ctx.switch_to(header_block);
                let loop_val = ctx
                    .push_instr(Instruction::ReadLocal {
                        local: loop_local,
                        ty: IrType::U64,
                    })
                    .expect("ReadLocal produces value");
                let end_read = ctx
                    .push_instr(Instruction::ReadLocal {
                        local: end_local,
                        ty: IrType::U64,
                    })
                    .expect("ReadLocal produces value");
                let cond_val = ctx
                    .push_instr(Instruction::Lt {
                        ty: IrType::U64,
                        lhs: loop_val,
                        rhs: end_read,
                    })
                    .expect("Lt produces value");
                ctx.set_terminator(Terminator::Branch {
                    condition: cond_val,
                    then_block: body_block,
                    else_block: exit_block,
                });

                // gövde
                ctx.switch_to(body_block);
                ctx.push_scope();
                ctx.define_local(var, loop_local, IrType::U64);
                for s in body {
                    self.lower_stmt(s, ctx)?;
                }
                ctx.pop_scope();
                if !ctx.current_block_terminated() {
                    // i += 1
                    let cur = ctx
                        .push_instr(Instruction::ReadLocal {
                            local: loop_local,
                            ty: IrType::U64,
                        })
                        .expect("ReadLocal produces value");
                    let one = ctx
                        .push_instr(Instruction::Const {
                            ty: IrType::U64,
                            value: 1,
                        })
                        .expect("Const produces value");
                    let next = ctx
                        .push_instr(Instruction::Add {
                            ty: IrType::U64,
                            lhs: cur,
                            rhs: one,
                        })
                        .expect("Add produces value");
                    ctx.push_instr(Instruction::WriteLocal {
                        local: loop_local,
                        value: next,
                    });
                    ctx.set_terminator(Terminator::Jump(header_block));
                }

                ctx.switch_to(exit_block);
            }

            Stmt::Return(maybe_expr) => {
                let ret_val = match maybe_expr {
                    Some(e) => {
                        let (v, _) = self.lower_expr_value(e, ctx)?;
                        Some(v)
                    }
                    None => None,
                };
                ctx.set_terminator(Terminator::Return(ret_val));
            }

            Stmt::Emit(event_name, args) => {
                let mut arg_vals = Vec::new();
                for arg in args {
                    let (v, _) = self.lower_expr_value(arg, ctx)?;
                    arg_vals.push(v);
                }
                ctx.push_instr(Instruction::Emit {
                    event_name: event_name.clone(),
                    args: arg_vals,
                });
            }

            Stmt::Expr(expr) => {
                // Void çağrılar dahil tüm ifadeler; sonuç yok sayılır.
                self.lower_expr(expr, ctx)?;
            }

            Stmt::MappingWrite(_, _, _) => {
                return Err(LoweringError::UnsupportedNode(
                    "MappingWrite — henüz IR lowering'de desteklenmiyor".into(),
                ));
            }
        }

        Ok(())
    }

    // ── İfade lowering ────────────────────────────────────────────────────

    /// İfadeyi lower eder ve `Option<(ValueId, IrType)>` döndürür.
    /// Void çağrılar için `None` döner; diğer tüm ifadeler `Some(...)` döner.
    fn lower_expr(
        &mut self,
        expr: &Expr,
        ctx: &mut FnCtx,
    ) -> Result<Option<(ValueId, IrType)>, LoweringError> {
        match expr {
            Expr::Int(v) => {
                let val = ctx
                    .push_instr(Instruction::Const {
                        ty: IrType::U64,
                        value: *v,
                    })
                    .expect("Const produces value");
                Ok(Some((val, IrType::U64)))
            }

            Expr::Ident(name) => {
                let (local, ty) = ctx
                    .lookup_local(name)
                    .ok_or_else(|| LoweringError::UndefinedVariable(name.clone()))?;
                let val = ctx
                    .push_instr(Instruction::ReadLocal {
                        local,
                        ty: ty.clone(),
                    })
                    .expect("ReadLocal produces value");
                Ok(Some((val, ty)))
            }

            Expr::Binary(lhs, op, rhs) => {
                let (l_val, l_ty) = self.lower_expr_value(lhs, ctx)?;
                let (r_val, _) = self.lower_expr_value(rhs, ctx)?;

                let (instr, res_ty) = match op {
                    BinOp::Add => (
                        Instruction::Add {
                            ty: l_ty.clone(),
                            lhs: l_val,
                            rhs: r_val,
                        },
                        l_ty,
                    ),
                    BinOp::Sub => (
                        Instruction::Sub {
                            ty: l_ty.clone(),
                            lhs: l_val,
                            rhs: r_val,
                        },
                        l_ty,
                    ),
                    BinOp::Mul => (
                        Instruction::Mul {
                            ty: l_ty.clone(),
                            lhs: l_val,
                            rhs: r_val,
                        },
                        l_ty,
                    ),
                    BinOp::Div => (
                        Instruction::Div {
                            ty: l_ty.clone(),
                            lhs: l_val,
                            rhs: r_val,
                        },
                        l_ty,
                    ),
                    BinOp::Eq => (
                        Instruction::IrEq {
                            ty: l_ty,
                            lhs: l_val,
                            rhs: r_val,
                        },
                        IrType::Bool,
                    ),
                    BinOp::Neq => (
                        Instruction::IrNe {
                            ty: l_ty,
                            lhs: l_val,
                            rhs: r_val,
                        },
                        IrType::Bool,
                    ),
                    BinOp::Lt => (
                        Instruction::Lt {
                            ty: l_ty,
                            lhs: l_val,
                            rhs: r_val,
                        },
                        IrType::Bool,
                    ),
                    BinOp::Lte => (
                        Instruction::Le {
                            ty: l_ty,
                            lhs: l_val,
                            rhs: r_val,
                        },
                        IrType::Bool,
                    ),
                    BinOp::Gt => (
                        Instruction::Gt {
                            ty: l_ty,
                            lhs: l_val,
                            rhs: r_val,
                        },
                        IrType::Bool,
                    ),
                    BinOp::Gte => (
                        Instruction::Ge {
                            ty: l_ty,
                            lhs: l_val,
                            rhs: r_val,
                        },
                        IrType::Bool,
                    ),
                };

                let val = ctx.push_instr(instr).expect("binary op produces value");
                Ok(Some((val, res_ty)))
            }

            Expr::StorageRead(name) => {
                let slot = *self
                    .storage_slots
                    .get(name)
                    .ok_or_else(|| LoweringError::UndefinedVariable(name.clone()))?;
                let val = ctx
                    .push_instr(Instruction::StateRead {
                        ty: IrType::U64,
                        slot,
                    })
                    .expect("StateRead produces value");
                Ok(Some((val, IrType::U64)))
            }

            Expr::Call(name, args) => self.lower_call(name, args, ctx),

            Expr::FieldAccess(base_expr, field) => {
                let (base_val, base_ty) = self.lower_expr_value(base_expr, ctx)?;
                let struct_name = match &base_ty {
                    IrType::Struct(n) => n.clone(),
                    _ => {
                        return Err(LoweringError::UnsupportedNode(
                            "field access on non-struct type".into(),
                        ))
                    }
                };
                let fields = self
                    .struct_field_order
                    .get(&struct_name)
                    .ok_or_else(|| LoweringError::UnresolvedType(struct_name.clone()))?;
                let idx = fields.iter().position(|f| f == field).ok_or_else(|| {
                    LoweringError::UnsupportedNode(format!(
                        "struct {struct_name} has no field {field}"
                    ))
                })?;
                let offset = (idx * 8) as i64;
                let val = ctx
                    .push_instr(Instruction::Load {
                        ty: IrType::U64,
                        base: base_val,
                        offset,
                    })
                    .expect("Load produces value");
                Ok(Some((val, IrType::U64)))
            }

            Expr::StructLiteral(_, _) => Err(LoweringError::UnsupportedNode(
                "struct literal — heap allocation modeli henüz IR'a eklenmedi".into(),
            )),

            Expr::MappingRead(_, _) => Err(LoweringError::UnsupportedNode(
                "MappingRead — henüz IR lowering'de desteklenmiyor".into(),
            )),
        }
    }

    /// İfadeyi lower eder ve değer üretmesini zorunlu tutar.
    /// Void ifadeler (void fonksiyon çağrısı) hata döndürür.
    fn lower_expr_value(
        &mut self,
        expr: &Expr,
        ctx: &mut FnCtx,
    ) -> Result<(ValueId, IrType), LoweringError> {
        self.lower_expr(expr, ctx)?
            .ok_or_else(|| LoweringError::UnsupportedNode("void expression used as value".into()))
    }

    /// Fonksiyon çağrısını lower eder. Built-in'ler özel olarak ele alınır.
    fn lower_call(
        &mut self,
        name: &str,
        args: &[Expr],
        ctx: &mut FnCtx,
    ) -> Result<Option<(ValueId, IrType)>, LoweringError> {
        // Context built-in'leri
        if name == "msg::sender" {
            let val = ctx
                .push_instr(Instruction::ContextRead {
                    kind: ContextKind::Sender,
                })
                .expect("ContextRead produces value");
            return Ok(Some((val, IrType::U64)));
        }
        if name == "msg::nonce" {
            let val = ctx
                .push_instr(Instruction::ContextRead {
                    kind: ContextKind::Nonce,
                })
                .expect("ContextRead produces value");
            return Ok(Some((val, IrType::U64)));
        }
        if name == "block::number" {
            let val = ctx
                .push_instr(Instruction::ContextRead {
                    kind: ContextKind::BlockHeight,
                })
                .expect("ContextRead produces value");
            return Ok(Some((val, IrType::U64)));
        }

        // Poseidon built-in
        if name == "poseidon" {
            if args.len() != 2 {
                return Err(LoweringError::UnsupportedNode(
                    "poseidon() exactly 2 arguments required".into(),
                ));
            }
            let (lhs, _) = self.lower_expr_value(&args[0], ctx)?;
            let (rhs, _) = self.lower_expr_value(&args[1], ctx)?;
            let val = ctx
                .push_instr(Instruction::Poseidon { lhs, rhs })
                .expect("Poseidon produces value");
            return Ok(Some((val, IrType::U64)));
        }

        // verify_merkle_proof — henüz desteklenmiyor
        if name == "verify_merkle_proof" {
            return Err(LoweringError::UnsupportedNode(
                "verify_merkle_proof — henüz IR lowering'de desteklenmiyor".into(),
            ));
        }

        // Kullanıcı tanımlı fonksiyon
        let fid = self
            .function_ids
            .get(name)
            .copied()
            .ok_or_else(|| LoweringError::UndefinedFunction(name.to_string()))?;

        let mut arg_vals = Vec::new();
        for arg in args {
            let (v, _) = self.lower_expr_value(arg, ctx)?;
            arg_vals.push(v);
        }

        // Dönüş tipini sema'dan al.
        let ret_ty = if let Some((_, ret)) = self.sema.functions.get(name) {
            from_sema_type(ret)?
        } else {
            IrType::Void
        };

        let instr = Instruction::Call {
            function: fid,
            args: arg_vals,
            ret_ty: ret_ty.clone(),
        };
        let result = ctx.push_instr(instr);

        if ret_ty == IrType::Void {
            Ok(None)
        } else {
            Ok(Some((
                result.expect("non-void call produces value"),
                ret_ty,
            )))
        }
    }
}

// ─── Fonksiyon inşa bağlamı ───────────────────────────────────────────────

/// Tek bir fonksiyonun lowering sırasında tutulan mutable durum.
struct FnCtx {
    #[allow(dead_code)]
    id: FunctionId,
    ret_ty: IrType,
    next_value: u32,
    next_block: u32,
    blocks: Vec<BasicBlock>,
    current_block: BlockId,
    /// Kapsam yığını: ad → (LocalId, IrType). Dıştakiler önce aranır.
    scopes: Vec<HashMap<String, (LocalId, IrType)>>,
    /// Yerel slot türleri, LocalId indeksiyle.
    locals: Vec<IrType>,
}

impl FnCtx {
    fn new(id: FunctionId, ret_ty: IrType) -> Self {
        let entry = BasicBlock {
            id: BlockId(0),
            instrs: vec![],
            terminator: None,
        };
        FnCtx {
            id,
            ret_ty,
            next_value: 0,
            next_block: 1,
            blocks: vec![entry],
            current_block: BlockId(0),
            scopes: vec![HashMap::new()],
            locals: vec![],
        }
    }

    fn fresh_value(&mut self) -> ValueId {
        let v = ValueId(self.next_value);
        self.next_value += 1;
        v
    }

    fn fresh_block(&mut self) -> BlockId {
        let b = BlockId(self.next_block);
        self.next_block += 1;
        self.blocks.push(BasicBlock {
            id: b,
            instrs: vec![],
            terminator: None,
        });
        b
    }

    fn fresh_local(&mut self, ty: IrType) -> LocalId {
        let l = LocalId(self.locals.len() as u32);
        self.locals.push(ty);
        l
    }

    /// Instruction'ı mevcut bloğa ekler; sonuç ValueId varsa döndürür.
    fn push_instr(&mut self, instr: Instruction) -> Option<ValueId> {
        let result = instr.result_type().map(|_| {
            let v = ValueId(self.next_value);
            self.next_value += 1;
            v
        });
        let block = self.block_mut(self.current_block);
        block.instrs.push(InstrNode { result, instr });
        result
    }

    fn set_terminator(&mut self, term: Terminator) {
        self.block_mut(self.current_block).terminator = Some(term);
    }

    fn switch_to(&mut self, block: BlockId) {
        self.current_block = block;
    }

    fn current_block_terminated(&self) -> bool {
        let id = self.current_block;
        self.blocks
            .iter()
            .find(|b| b.id == id)
            .is_some_and(|b| b.terminator.is_some())
    }

    fn block_mut(&mut self, id: BlockId) -> &mut BasicBlock {
        self.blocks
            .iter_mut()
            .find(|b| b.id == id)
            .expect("block_mut: block not found")
    }

    fn define_local(&mut self, name: &str, local: LocalId, ty: IrType) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), (local, ty));
        }
    }

    fn lookup_local(&self, name: &str) -> Option<(LocalId, IrType)> {
        for scope in self.scopes.iter().rev() {
            if let Some(entry) = scope.get(name) {
                return Some(entry.clone());
            }
        }
        None
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    #[allow(dead_code)]
    fn ret_ty(&self) -> &IrType {
        &self.ret_ty
    }
}

// ─── Testler ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;
    use crate::sema::SemanticAnalyzer;

    fn lower_source(src: &str) -> Result<IrProgram, LoweringError> {
        let mut p = Parser::new(src);
        let contract = p.parse_contract().expect("parse failed");
        let mut sema = SemanticAnalyzer::new();
        sema.analyze(&contract).expect("sema failed");
        lower_contract(&contract, &sema)
    }

    // ── 1. Aritmetik AST → IR ─────────────────────────────────────────────
    #[test]
    fn test_arithmetic_lowering() {
        let prog = lower_source(
            "contract T { pub fn main() -> u64 { let a = 10; let b = 20; return a + b; } }",
        )
        .expect("lowering failed");
        assert_eq!(prog.functions.len(), 1);
        let f = &prog.functions[0];
        // entry block'ta Const 10, WriteLocal, Const 20, WriteLocal, ReadLocal, ReadLocal, Add,
        // ReadLocal (ret), Return gibi instruction'lar bekleniyor.
        let bb0 = &f.blocks[0];
        let has_add = bb0
            .instrs
            .iter()
            .any(|n| matches!(&n.instr, Instruction::Add { .. }));
        assert!(has_add, "Add instruction missing");
        assert!(matches!(bb0.terminator, Some(Terminator::Return(Some(_)))));
    }

    // ── 2. Karşılaştırma ─────────────────────────────────────────────────
    #[test]
    fn test_comparison_lowering() {
        let prog = lower_source(
            "contract T { pub fn main() -> u64 { let a = 5; let b = 10; let c = a < b; return a; } }",
        )
        .expect("lowering failed");
        let f = &prog.functions[0];
        let bb0 = &f.blocks[0];
        let has_lt = bb0
            .instrs
            .iter()
            .any(|n| matches!(&n.instr, Instruction::Lt { .. }));
        assert!(has_lt, "Lt instruction missing");
    }

    // ── 3. if/else CFG ───────────────────────────────────────────────────
    #[test]
    fn test_if_else_cfg() {
        let prog = lower_source(
            r"contract T {
                pub fn main() -> u64 {
                    let x = 0;
                    if (x == 0) {
                        return 1;
                    } else {
                        return 2;
                    }
                }
            }",
        )
        .expect("lowering failed");
        let f = &prog.functions[0];
        // En az 3 blok: entry (branch), then, else + merge (opsiyonel)
        assert!(
            f.blocks.len() >= 3,
            "expected at least 3 blocks, got {}",
            f.blocks.len()
        );
        let entry = &f.blocks[0];
        assert!(
            matches!(entry.terminator, Some(Terminator::Branch { .. })),
            "entry block must terminate with Branch"
        );
    }

    // ── 4. Fonksiyon çağrısı ─────────────────────────────────────────────
    #[test]
    fn test_function_call_lowering() {
        let prog = lower_source(
            r"contract T {
                fn add(a: u64, b: u64) -> u64 { return a + b; }
                pub fn main() -> u64 { return add(1, 2); }
            }",
        )
        .expect("lowering failed");
        assert_eq!(prog.functions.len(), 2);
        let main_fn = prog
            .functions
            .iter()
            .find(|f| f.name == "main")
            .expect("main not found");
        let has_call = main_fn
            .blocks
            .iter()
            .flat_map(|b| &b.instrs)
            .any(|n| matches!(&n.instr, Instruction::Call { .. }));
        assert!(has_call, "Call instruction missing in main");
    }

    // ── 5. context.sender ────────────────────────────────────────────────
    #[test]
    fn test_context_sender_lowering() {
        let prog = lower_source(
            "contract T { pub fn main() -> u64 { let s = msg::sender(); return s; } }",
        )
        .expect("lowering failed");
        let f = &prog.functions[0];
        let has_sender = f.blocks.iter().flat_map(|b| &b.instrs).any(|n| {
            matches!(
                &n.instr,
                Instruction::ContextRead {
                    kind: ContextKind::Sender
                }
            )
        });
        assert!(has_sender, "ContextRead(Sender) missing");
    }

    // ── 6. Storage read / write ───────────────────────────────────────────
    #[test]
    fn test_storage_read_write_lowering() {
        let prog = lower_source(
            r"contract T {
                storage { counter: u64, }
                pub fn main() {
                    let v = storage::counter;
                    storage::counter = v;
                }
            }",
        )
        .expect("lowering failed");
        let f = &prog.functions[0];
        let instrs: Vec<_> = f.blocks.iter().flat_map(|b| &b.instrs).collect();
        let has_read = instrs
            .iter()
            .any(|n| matches!(&n.instr, Instruction::StateRead { .. }));
        let has_write = instrs
            .iter()
            .any(|n| matches!(&n.instr, Instruction::StateWrite { .. }));
        assert!(has_read, "StateRead missing");
        assert!(has_write, "StateWrite missing");
    }

    // ── 7. constrain → Assert ────────────────────────────────────────────
    #[test]
    fn test_constrain_becomes_assert() {
        let prog = lower_source("contract T { pub fn main() { let x = 1; constrain(x); } }")
            .expect("lowering failed");
        let f = &prog.functions[0];
        let has_assert = f
            .blocks
            .iter()
            .flat_map(|b| &b.instrs)
            .any(|n| matches!(&n.instr, Instruction::Assert { .. }));
        assert!(has_assert, "Assert instruction missing");
    }

    // ── 8. Deterministik IR dump ──────────────────────────────────────────
    #[test]
    fn test_deterministic_dump() {
        let src = "contract T { pub fn main() -> u64 { let a = 1; return a; } }";
        let prog1 = lower_source(src).expect("1st lower failed");
        let prog2 = lower_source(src).expect("2nd lower failed");
        let dump1 = format!("{prog1}");
        let dump2 = format!("{prog2}");
        assert_eq!(dump1, dump2, "IR dump is non-deterministic");
    }

    // ── 9. while döngüsü ─────────────────────────────────────────────────
    #[test]
    fn test_while_lowering() {
        let prog = lower_source(
            r"contract T {
                pub fn main() -> u64 {
                    let i = 0;
                    while (i < 10) {
                        i = i + 1;
                    }
                    return i;
                }
            }",
        )
        .expect("lowering failed");
        let f = &prog.functions[0];
        // while → header (branch) + body (jump back) + exit
        assert!(
            f.blocks.len() >= 4,
            "expected >= 4 blocks for while, got {}",
            f.blocks.len()
        );
    }

    // ── 10. emit deyimi ──────────────────────────────────────────────────
    #[test]
    fn test_emit_lowering() {
        let prog = lower_source("contract T { pub fn main() { let x = 42; emit Transfer(x); } }")
            .expect("lowering failed");
        let f = &prog.functions[0];
        let has_emit = f
            .blocks
            .iter()
            .flat_map(|b| &b.instrs)
            .any(|n| matches!(&n.instr, Instruction::Emit { .. }));
        assert!(has_emit, "Emit instruction missing");
    }

    // ── 11. Poseidon çağrısı ──────────────────────────────────────────────
    #[test]
    fn test_poseidon_lowering() {
        let prog = lower_source(
            "contract T { pub fn main() -> u64 { let h = poseidon(1, 2); return h; } }",
        )
        .expect("lowering failed");
        let f = &prog.functions[0];
        let has_poseidon = f
            .blocks
            .iter()
            .flat_map(|b| &b.instrs)
            .any(|n| matches!(&n.instr, Instruction::Poseidon { .. }));
        assert!(has_poseidon, "Poseidon instruction missing");
    }
}
