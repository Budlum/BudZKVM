//! BudIR yapısal doğrulayıcı (verifier).
//!
//! Verifier, lowering veya elle yapılan IR inşaasındaki hataları erken
//! yakalar. Bir LLVM IR verifier'ına benzer şekilde, backend'e ulaşmadan
//! önce IR değişmezlerini kontrol eder.
//!
//! ## Kontrol edilen değişmezler
//! 1. Tüm kullanılan `ValueId`'ler tanımlanmış mı? (SSA single-def)
//! 2. Aynı `ValueId` birden fazla kez tanımlanmış mı?
//! 3. Terminator'da ve instruction'larda başvurulan `BlockId`'ler var mı?
//! 4. Her blok tam olarak bir Terminator ile bitiyor mu?
//! 5. `Return` değerinin tipi fonksiyon dönüş tipiyle uyumlu mu?
//! 6. `Branch` koşulu `Bool` mu?
//! 7. Aritmetik operandların tipleri eşleşiyor mu?
//! 8. `ReadLocal` / `WriteLocal` içindeki `LocalId`'ler geçerli mi?
//! 9. Parametre `ValueId`'leri de geçerli olarak sayılıyor mu?

use std::collections::{HashMap, HashSet};

use super::{
    BasicBlock, BlockId, FunctionId, Instruction, IrFunction, IrProgram, IrType, LocalId,
    Terminator, ValueId,
};

// ─── Hata tipi ────────────────────────────────────────────────────────────

/// IR doğrulama hatası.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// Referans verilen `ValueId` hiçbir yerde tanımlanmamış.
    UndefinedValue { value: ValueId, in_function: String },
    /// Aynı `ValueId` birden fazla kez tanımlanmış.
    DuplicateValueDef { value: ValueId, in_function: String },
    /// Referans verilen `BlockId` fonksiyon içinde mevcut değil.
    UndefinedBlock { block: BlockId, in_function: String },
    /// Bir blok Terminator'sız bitiyor.
    MissingTerminator { block: BlockId, in_function: String },
    /// `Return` değerinin tipi fonksiyon dönüş tipiyle uyumsuz.
    ReturnTypeMismatch {
        expected: IrType,
        got: Option<IrType>,
        in_function: String,
    },
    /// `Branch` koşulunun tipi `Bool` değil.
    BranchConditionNotBool {
        value: ValueId,
        actual_ty: IrType,
        in_function: String,
    },
    /// Aritmetik veya karşılaştırma operandlarının tipleri eşleşmiyor.
    OperandTypeMismatch {
        lhs_ty: IrType,
        rhs_ty: IrType,
        in_function: String,
    },
    /// `LocalId` fonksiyonun `locals` listesi dışında.
    UndefinedLocal { local: LocalId, in_function: String },
    /// Fonksiyon bulunamadı (program düzeyinde çapraz referans).
    UndefinedFunction {
        function: FunctionId,
        in_caller: String,
    },
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::UndefinedValue { value, in_function } => {
                write!(f, "fn {in_function}: undefined value %{}", value.0)
            }
            VerifyError::DuplicateValueDef { value, in_function } => {
                write!(
                    f,
                    "fn {in_function}: value %{} defined more than once",
                    value.0
                )
            }
            VerifyError::UndefinedBlock { block, in_function } => {
                write!(f, "fn {in_function}: undefined block bb{}", block.0)
            }
            VerifyError::MissingTerminator { block, in_function } => {
                write!(f, "fn {in_function}: block bb{} has no terminator", block.0)
            }
            VerifyError::ReturnTypeMismatch {
                expected,
                got,
                in_function,
            } => {
                write!(
                    f,
                    "fn {in_function}: return type mismatch — expected {expected:?}, got {got:?}"
                )
            }
            VerifyError::BranchConditionNotBool {
                value,
                actual_ty,
                in_function,
            } => {
                write!(
                    f,
                    "fn {in_function}: branch condition %{} has type {actual_ty:?} (expected Bool)",
                    value.0
                )
            }
            VerifyError::OperandTypeMismatch {
                lhs_ty,
                rhs_ty,
                in_function,
            } => {
                write!(
                    f,
                    "fn {in_function}: operand type mismatch — lhs {lhs_ty:?}, rhs {rhs_ty:?}"
                )
            }
            VerifyError::UndefinedLocal { local, in_function } => {
                write!(f, "fn {in_function}: undefined local l{}", local.0)
            }
            VerifyError::UndefinedFunction {
                function,
                in_caller,
            } => {
                write!(
                    f,
                    "fn {in_caller}: call to undefined function id {}",
                    function.0
                )
            }
        }
    }
}

impl std::error::Error for VerifyError {}

// ─── Genel giriş noktası ──────────────────────────────────────────────────

/// Tüm programı doğrular. İlk bulunan hatayı döndürür.
pub fn verify_program(program: &IrProgram) -> Result<(), VerifyError> {
    let valid_fids: HashSet<FunctionId> = program.functions.iter().map(|f| f.id).collect();

    for func in &program.functions {
        verify_function(func, &valid_fids)?;
    }
    Ok(())
}

/// Tek bir fonksiyonu doğrular.
pub fn verify_function(
    func: &IrFunction,
    valid_fids: &HashSet<FunctionId>,
) -> Result<(), VerifyError> {
    let fname = &func.name;
    let local_count = func.locals.len();
    let valid_blocks: HashSet<BlockId> = func.blocks.iter().map(|b| b.id).collect();

    // ── 1. Tanımlı ValueId'leri topla: önce parametreler ─────────────────
    let mut defined: HashMap<ValueId, IrType> = HashMap::new();
    let mut defined_order: Vec<ValueId> = Vec::new();

    for (vid, ty) in &func.params {
        if defined.insert(*vid, ty.clone()).is_some() {
            return Err(VerifyError::DuplicateValueDef {
                value: *vid,
                in_function: fname.clone(),
            });
        }
        defined_order.push(*vid);
    }

    // ── 2. Tüm instruction'ları önceden tara (tanım seti) ─────────────────
    // (Verifier basit bir "tüm tanımlar" seti kullanır; dominance analizi
    //  değil. Bu pre-SSA IR için yeterlidir.)
    for block in &func.blocks {
        for node in &block.instrs {
            if let Some(vid) = node.result {
                if defined
                    .insert(vid, node.instr.result_type().unwrap())
                    .is_some()
                {
                    return Err(VerifyError::DuplicateValueDef {
                        value: vid,
                        in_function: fname.clone(),
                    });
                }
                defined_order.push(vid);
            }
        }
    }

    // ── 3. Her bloğu tek tek doğrula ──────────────────────────────────────
    for block in &func.blocks {
        verify_block(
            block,
            func,
            fname,
            &defined,
            &valid_blocks,
            valid_fids,
            local_count,
        )?;
    }

    Ok(())
}

fn verify_block(
    block: &BasicBlock,
    func: &IrFunction,
    fname: &str,
    defined: &HashMap<ValueId, IrType>,
    valid_blocks: &HashSet<BlockId>,
    valid_fids: &HashSet<FunctionId>,
    local_count: usize,
) -> Result<(), VerifyError> {
    // ── Instruction'ları kontrol et ───────────────────────────────────────
    for node in &block.instrs {
        verify_instr(
            &node.instr,
            fname,
            defined,
            valid_blocks,
            valid_fids,
            local_count,
        )?;
    }

    // ── Terminator kontrolü ───────────────────────────────────────────────
    let term = block
        .terminator
        .as_ref()
        .ok_or_else(|| VerifyError::MissingTerminator {
            block: block.id,
            in_function: fname.to_string(),
        })?;

    match term {
        Terminator::Jump(target) => {
            check_block_ref(*target, fname, valid_blocks)?;
        }
        Terminator::Branch {
            condition,
            then_block,
            else_block,
        } => {
            // Koşul tipi Bool olmalı.
            let cond_ty = defined
                .get(condition)
                .ok_or_else(|| VerifyError::UndefinedValue {
                    value: *condition,
                    in_function: fname.to_string(),
                })?;
            if *cond_ty != IrType::Bool {
                return Err(VerifyError::BranchConditionNotBool {
                    value: *condition,
                    actual_ty: cond_ty.clone(),
                    in_function: fname.to_string(),
                });
            }
            check_block_ref(*then_block, fname, valid_blocks)?;
            check_block_ref(*else_block, fname, valid_blocks)?;
        }
        Terminator::Return(maybe_val) => {
            let got_ty = match maybe_val {
                Some(vid) => {
                    let ty = defined
                        .get(vid)
                        .ok_or_else(|| VerifyError::UndefinedValue {
                            value: *vid,
                            in_function: fname.to_string(),
                        })?
                        .clone();
                    Some(ty)
                }
                None => None,
            };

            // Dönüş tipi uyumu kontrolü (Void = None, non-void = Some).
            let expected = &func.ret_ty;
            let matches = match (expected, &got_ty) {
                (IrType::Void, None) => true,
                (IrType::Void, Some(_)) => false,
                (_, None) => false,
                (exp, Some(got)) => exp == got,
            };
            if !matches {
                return Err(VerifyError::ReturnTypeMismatch {
                    expected: expected.clone(),
                    got: got_ty,
                    in_function: fname.to_string(),
                });
            }
        }
        Terminator::Unreachable => {}
    }

    Ok(())
}

fn verify_instr(
    instr: &Instruction,
    fname: &str,
    defined: &HashMap<ValueId, IrType>,
    valid_blocks: &HashSet<BlockId>,
    valid_fids: &HashSet<FunctionId>,
    local_count: usize,
) -> Result<(), VerifyError> {
    match instr {
        Instruction::Const { .. } => {}

        Instruction::Add { lhs, rhs, ty }
        | Instruction::Sub { lhs, rhs, ty }
        | Instruction::Mul { lhs, rhs, ty }
        | Instruction::Div { lhs, rhs, ty } => {
            let l_ty = check_value_ref(*lhs, fname, defined)?;
            let r_ty = check_value_ref(*rhs, fname, defined)?;
            if &l_ty != ty || &r_ty != ty {
                return Err(VerifyError::OperandTypeMismatch {
                    lhs_ty: l_ty,
                    rhs_ty: r_ty,
                    in_function: fname.to_string(),
                });
            }
        }

        Instruction::IrEq { lhs, rhs, ty }
        | Instruction::IrNe { lhs, rhs, ty }
        | Instruction::Lt { lhs, rhs, ty }
        | Instruction::Le { lhs, rhs, ty }
        | Instruction::Gt { lhs, rhs, ty }
        | Instruction::Ge { lhs, rhs, ty } => {
            let l_ty = check_value_ref(*lhs, fname, defined)?;
            let r_ty = check_value_ref(*rhs, fname, defined)?;
            if &l_ty != ty || &r_ty != ty {
                return Err(VerifyError::OperandTypeMismatch {
                    lhs_ty: l_ty,
                    rhs_ty: r_ty,
                    in_function: fname.to_string(),
                });
            }
        }

        Instruction::Load { base, .. } => {
            check_value_ref(*base, fname, defined)?;
        }
        Instruction::Store { base, value, .. } => {
            check_value_ref(*base, fname, defined)?;
            check_value_ref(*value, fname, defined)?;
        }

        Instruction::StateRead { .. } => {}
        Instruction::StateWrite { value, .. } => {
            check_value_ref(*value, fname, defined)?;
        }

        Instruction::ReadLocal { local, .. } => {
            check_local_ref(*local, fname, local_count)?;
        }
        Instruction::WriteLocal { local, value } => {
            check_local_ref(*local, fname, local_count)?;
            check_value_ref(*value, fname, defined)?;
        }

        Instruction::Call { function, args, .. } => {
            if !valid_fids.contains(function) {
                return Err(VerifyError::UndefinedFunction {
                    function: *function,
                    in_caller: fname.to_string(),
                });
            }
            for &arg in args {
                check_value_ref(arg, fname, defined)?;
            }
        }

        Instruction::Assert { condition } => {
            check_value_ref(*condition, fname, defined)?;
        }
        Instruction::Poseidon { lhs, rhs } => {
            check_value_ref(*lhs, fname, defined)?;
            check_value_ref(*rhs, fname, defined)?;
        }
        Instruction::Emit { args, .. } => {
            for &arg in args {
                check_value_ref(arg, fname, defined)?;
            }
        }
        Instruction::ContextRead { .. } => {}
    }

    let _ = valid_blocks; // ileride daha ayrıntılı CFG kontrolü için
    Ok(())
}

// ─── Yardımcılar ──────────────────────────────────────────────────────────

fn check_value_ref(
    vid: ValueId,
    fname: &str,
    defined: &HashMap<ValueId, IrType>,
) -> Result<IrType, VerifyError> {
    defined
        .get(&vid)
        .cloned()
        .ok_or_else(|| VerifyError::UndefinedValue {
            value: vid,
            in_function: fname.to_string(),
        })
}

fn check_block_ref(
    bid: BlockId,
    fname: &str,
    valid_blocks: &HashSet<BlockId>,
) -> Result<(), VerifyError> {
    if valid_blocks.contains(&bid) {
        Ok(())
    } else {
        Err(VerifyError::UndefinedBlock {
            block: bid,
            in_function: fname.to_string(),
        })
    }
}

fn check_local_ref(local: LocalId, fname: &str, local_count: usize) -> Result<(), VerifyError> {
    if local.index() < local_count {
        Ok(())
    } else {
        Err(VerifyError::UndefinedLocal {
            local,
            in_function: fname.to_string(),
        })
    }
}

// ─── Testler ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        BasicBlock, BlockId, FunctionId, InstrNode, Instruction, IrFunction, IrProgram, IrType,
        LocalId, Terminator, ValueId,
    };

    /// Minimal geçerli program inşa eden yardımcı.
    fn valid_program() -> IrProgram {
        // fn test() -> u64 {
        //   %0 = const.u64 42
        //   ret %0
        // }
        let block = BasicBlock {
            id: BlockId::new(0),
            instrs: vec![InstrNode {
                result: Some(ValueId::new(0)),
                instr: Instruction::Const {
                    ty: IrType::U64,
                    value: 42,
                },
            }],
            terminator: Some(Terminator::Return(Some(ValueId::new(0)))),
        };
        let func = IrFunction {
            id: FunctionId(0),
            name: "test".to_string(),
            params: vec![],
            ret_ty: IrType::U64,
            blocks: vec![block],
            locals: vec![],
        };
        IrProgram {
            functions: vec![func],
            function_names: [("test".to_string(), FunctionId(0))].into(),
        }
    }

    // ── 9. Verifier geçerli IR'ı kabul eder ──────────────────────────────
    #[test]
    fn verifier_accepts_valid_program() {
        let prog = valid_program();
        assert!(verify_program(&prog).is_ok());
    }

    // ── 10. Verifier tanımsız ValueId'i reddeder ─────────────────────────
    #[test]
    fn verifier_rejects_undefined_value() {
        let block = BasicBlock {
            id: BlockId::new(0),
            instrs: vec![InstrNode {
                result: Some(ValueId::new(0)),
                instr: Instruction::Add {
                    ty: IrType::U64,
                    lhs: ValueId::new(99), // tanımsız!
                    rhs: ValueId::new(99), // tanımsız!
                },
            }],
            terminator: Some(Terminator::Return(Some(ValueId::new(0)))),
        };
        let func = IrFunction {
            id: FunctionId(0),
            name: "bad".to_string(),
            params: vec![],
            ret_ty: IrType::U64,
            blocks: vec![block],
            locals: vec![],
        };
        let prog = IrProgram {
            functions: vec![func],
            function_names: Default::default(),
        };
        let result = verify_program(&prog);
        assert!(
            matches!(result, Err(VerifyError::UndefinedValue { .. })),
            "expected UndefinedValue, got: {result:?}"
        );
    }

    // ── 11. Verifier tanımsız BlockId'i reddeder ─────────────────────────
    #[test]
    fn verifier_rejects_undefined_block() {
        let block = BasicBlock {
            id: BlockId::new(0),
            instrs: vec![InstrNode {
                result: Some(ValueId::new(0)),
                instr: Instruction::Const {
                    ty: IrType::U64,
                    value: 1,
                },
            }],
            // bb999 yok!
            terminator: Some(Terminator::Jump(BlockId::new(999))),
        };
        let func = IrFunction {
            id: FunctionId(0),
            name: "bad".to_string(),
            params: vec![],
            ret_ty: IrType::Void,
            blocks: vec![block],
            locals: vec![],
        };
        let prog = IrProgram {
            functions: vec![func],
            function_names: Default::default(),
        };
        let result = verify_program(&prog);
        assert!(
            matches!(result, Err(VerifyError::UndefinedBlock { .. })),
            "expected UndefinedBlock, got: {result:?}"
        );
    }

    // ── 12. Verifier eksik terminator'ı reddeder ──────────────────────────
    #[test]
    fn verifier_rejects_missing_terminator() {
        let block = BasicBlock {
            id: BlockId::new(0),
            instrs: vec![],
            terminator: None, // eksik!
        };
        let func = IrFunction {
            id: FunctionId(0),
            name: "bad".to_string(),
            params: vec![],
            ret_ty: IrType::Void,
            blocks: vec![block],
            locals: vec![],
        };
        let prog = IrProgram {
            functions: vec![func],
            function_names: Default::default(),
        };
        assert!(matches!(
            verify_program(&prog),
            Err(VerifyError::MissingTerminator { .. })
        ));
    }

    // ── 13. Verifier Bool olmayan branch koşulunu reddeder ────────────────
    #[test]
    fn verifier_rejects_non_bool_branch_condition() {
        let bb1 = BasicBlock {
            id: BlockId::new(1),
            instrs: vec![],
            terminator: Some(Terminator::Return(None)),
        };
        let bb2 = BasicBlock {
            id: BlockId::new(2),
            instrs: vec![],
            terminator: Some(Terminator::Return(None)),
        };
        let block = BasicBlock {
            id: BlockId::new(0),
            instrs: vec![InstrNode {
                result: Some(ValueId::new(0)),
                // U64 döndürüyor, Bool değil
                instr: Instruction::Const {
                    ty: IrType::U64,
                    value: 1,
                },
            }],
            terminator: Some(Terminator::Branch {
                condition: ValueId::new(0), // U64, Bool değil!
                then_block: BlockId::new(1),
                else_block: BlockId::new(2),
            }),
        };
        let func = IrFunction {
            id: FunctionId(0),
            name: "bad".to_string(),
            params: vec![],
            ret_ty: IrType::Void,
            blocks: vec![block, bb1, bb2],
            locals: vec![],
        };
        let prog = IrProgram {
            functions: vec![func],
            function_names: Default::default(),
        };
        assert!(matches!(
            verify_program(&prog),
            Err(VerifyError::BranchConditionNotBool { .. })
        ),);
    }

    // ── 14. Verifier yanlış dönüş tipini reddeder ─────────────────────────
    #[test]
    fn verifier_rejects_wrong_return_type() {
        let block = BasicBlock {
            id: BlockId::new(0),
            instrs: vec![InstrNode {
                result: Some(ValueId::new(0)),
                instr: Instruction::Const {
                    ty: IrType::U64,
                    value: 5,
                },
            }],
            terminator: Some(Terminator::Return(Some(ValueId::new(0)))),
        };
        let func = IrFunction {
            id: FunctionId(0),
            name: "bad".to_string(),
            params: vec![],
            ret_ty: IrType::Void, // fonksiyon void döndürmeli ama U64 dönüyor
            blocks: vec![block],
            locals: vec![],
        };
        let prog = IrProgram {
            functions: vec![func],
            function_names: Default::default(),
        };
        assert!(matches!(
            verify_program(&prog),
            Err(VerifyError::ReturnTypeMismatch { .. })
        ));
    }

    // ── 15. Verifier çift tanımlı ValueId'i reddeder ──────────────────────
    #[test]
    fn verifier_rejects_duplicate_value_def() {
        let block = BasicBlock {
            id: BlockId::new(0),
            instrs: vec![
                InstrNode {
                    result: Some(ValueId::new(0)),
                    instr: Instruction::Const {
                        ty: IrType::U64,
                        value: 1,
                    },
                },
                InstrNode {
                    result: Some(ValueId::new(0)), // tekrar!
                    instr: Instruction::Const {
                        ty: IrType::U64,
                        value: 2,
                    },
                },
            ],
            terminator: Some(Terminator::Return(Some(ValueId::new(0)))),
        };
        let func = IrFunction {
            id: FunctionId(0),
            name: "bad".to_string(),
            params: vec![],
            ret_ty: IrType::U64,
            blocks: vec![block],
            locals: vec![],
        };
        let prog = IrProgram {
            functions: vec![func],
            function_names: Default::default(),
        };
        assert!(matches!(
            verify_program(&prog),
            Err(VerifyError::DuplicateValueDef { .. })
        ));
    }

    // ── 16. Verifier tanımsız LocalId'i reddeder ──────────────────────────
    #[test]
    fn verifier_rejects_undefined_local() {
        let block = BasicBlock {
            id: BlockId::new(0),
            instrs: vec![InstrNode {
                result: Some(ValueId::new(0)),
                instr: Instruction::ReadLocal {
                    local: LocalId::new(99), // locals listesi boş, geçersiz!
                    ty: IrType::U64,
                },
            }],
            terminator: Some(Terminator::Return(Some(ValueId::new(0)))),
        };
        let func = IrFunction {
            id: FunctionId(0),
            name: "bad".to_string(),
            params: vec![],
            ret_ty: IrType::U64,
            blocks: vec![block],
            locals: vec![], // boş — LocalId(99) geçersiz
        };
        let prog = IrProgram {
            functions: vec![func],
            function_names: Default::default(),
        };
        assert!(matches!(
            verify_program(&prog),
            Err(VerifyError::UndefinedLocal { .. })
        ));
    }

    // ── 17. Verifier fonksiyon parametrelerini tanımlı sayar ──────────────
    #[test]
    fn verifier_params_are_defined() {
        // fn add(a: u64, b: u64) -> u64 { ret %0 + %1 }  (parametreler %0, %1)
        let block = BasicBlock {
            id: BlockId::new(0),
            instrs: vec![InstrNode {
                result: Some(ValueId::new(2)),
                instr: Instruction::Add {
                    ty: IrType::U64,
                    lhs: ValueId::new(0), // param
                    rhs: ValueId::new(1), // param
                },
            }],
            terminator: Some(Terminator::Return(Some(ValueId::new(2)))),
        };
        let func = IrFunction {
            id: FunctionId(0),
            name: "add".to_string(),
            params: vec![
                (ValueId::new(0), IrType::U64),
                (ValueId::new(1), IrType::U64),
            ],
            ret_ty: IrType::U64,
            blocks: vec![block],
            locals: vec![],
        };
        let prog = IrProgram {
            functions: vec![func],
            function_names: Default::default(),
        };
        assert!(verify_program(&prog).is_ok());
    }
}
