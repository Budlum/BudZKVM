//! BudIR metin temsili (debug/test için).
//!
//! Bu production syntax değildir. Aynı input için her zaman byte-for-byte
//! aynı çıktı üretilir (deterministic). HashMap iterasyonu kullanılmaz;
//! tüm sıralama `BlockId` ve `ValueId` üzerinden yapılır.
//!
//! ## Örnek çıktı
//! ```text
//! fn main() -> u64 {
//! bb0:
//!     %0 = const.u64 10
//!     %1 = const.u64 20
//!     %2 = add.u64 %0, %1
//!     %3 = lt.u64 %2, %1
//!     br %3, bb1, bb2
//!
//! bb1:
//!     ret %2
//!
//! bb2:
//!     ret %0
//! }
//! ```

use std::fmt;

use super::{
    BasicBlock, ContextKind, Instruction, IrFunction, IrProgram, IrType, Terminator, ValueId,
};

// ─── IrProgram ────────────────────────────────────────────────────────────

impl fmt::Display for IrProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Fonksiyonları IrFunction sıralamasıyla yaz (deterministik).
        for (i, func) in self.functions.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{func}")?;
        }
        Ok(())
    }
}

// ─── IrFunction ───────────────────────────────────────────────────────────

impl fmt::Display for IrFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // İmza
        write!(f, "fn {}(", self.name)?;
        for (i, (vid, ty)) in self.params.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", fmt_value(*vid), fmt_type(ty))?;
        }
        write!(f, ") -> {} {{", fmt_type(&self.ret_ty))?;
        writeln!(f)?;

        // Yerel slotları varsa göster.
        if !self.locals.is_empty() {
            write!(f, "  ; locals:")?;
            for (i, ty) in self.locals.iter().enumerate() {
                write!(f, " l{}:{}", i, fmt_type(ty))?;
            }
            writeln!(f)?;
        }

        // Bloklar (BlockId sırasına göre zaten sıralı tutulur).
        for block in &self.blocks {
            write!(f, "{block}")?;
        }

        write!(f, "}}")
    }
}

// ─── BasicBlock ───────────────────────────────────────────────────────────

impl fmt::Display for BasicBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "bb{}:", self.id.0)?;
        for node in &self.instrs {
            match &node.result {
                Some(vid) => write!(f, "    {} = ", fmt_value(*vid))?,
                None => write!(f, "    ")?,
            }
            writeln!(f, "{}", fmt_instr(&node.instr))?;
        }
        if let Some(term) = &self.terminator {
            writeln!(f, "    {}", fmt_term(term))?;
        } else {
            writeln!(f, "    ; <no terminator — IR under construction>")?;
        }
        Ok(())
    }
}

// ─── Yardımcı formatlayıcılar ─────────────────────────────────────────────

fn fmt_value(v: ValueId) -> String {
    format!("%{}", v.0)
}

fn fmt_type(ty: &IrType) -> &'static str {
    match ty {
        IrType::U64 => "u64",
        IrType::Bool => "bool",
        IrType::Field => "field",
        IrType::Void => "void",
        IrType::Struct(_) => "struct",
    }
}

fn fmt_type_owned(ty: &IrType) -> String {
    match ty {
        IrType::Struct(name) => format!("struct.{name}"),
        other => fmt_type(other).to_string(),
    }
}

fn fmt_instr(instr: &Instruction) -> String {
    match instr {
        Instruction::Const { ty, value } => format!("const.{} {value}", fmt_type(ty)),

        Instruction::Add { ty, lhs, rhs } => format!(
            "add.{} {}, {}",
            fmt_type(ty),
            fmt_value(*lhs),
            fmt_value(*rhs)
        ),
        Instruction::Sub { ty, lhs, rhs } => format!(
            "sub.{} {}, {}",
            fmt_type(ty),
            fmt_value(*lhs),
            fmt_value(*rhs)
        ),
        Instruction::Mul { ty, lhs, rhs } => format!(
            "mul.{} {}, {}",
            fmt_type(ty),
            fmt_value(*lhs),
            fmt_value(*rhs)
        ),
        Instruction::Div { ty, lhs, rhs } => format!(
            "div.{} {}, {}",
            fmt_type(ty),
            fmt_value(*lhs),
            fmt_value(*rhs)
        ),

        Instruction::IrEq { ty, lhs, rhs } => format!(
            "eq.{} {}, {}",
            fmt_type(ty),
            fmt_value(*lhs),
            fmt_value(*rhs)
        ),
        Instruction::IrNe { ty, lhs, rhs } => format!(
            "ne.{} {}, {}",
            fmt_type(ty),
            fmt_value(*lhs),
            fmt_value(*rhs)
        ),
        Instruction::Lt { ty, lhs, rhs } => format!(
            "lt.{} {}, {}",
            fmt_type(ty),
            fmt_value(*lhs),
            fmt_value(*rhs)
        ),
        Instruction::Le { ty, lhs, rhs } => format!(
            "le.{} {}, {}",
            fmt_type(ty),
            fmt_value(*lhs),
            fmt_value(*rhs)
        ),
        Instruction::Gt { ty, lhs, rhs } => format!(
            "gt.{} {}, {}",
            fmt_type(ty),
            fmt_value(*lhs),
            fmt_value(*rhs)
        ),
        Instruction::Ge { ty, lhs, rhs } => format!(
            "ge.{} {}, {}",
            fmt_type(ty),
            fmt_value(*lhs),
            fmt_value(*rhs)
        ),

        Instruction::Load { ty, base, offset } => {
            format!("load.{} {}[{offset}]", fmt_type(ty), fmt_value(*base))
        }
        Instruction::Store {
            base,
            offset,
            value,
        } => format!(
            "store {}[{offset}], {}",
            fmt_value(*base),
            fmt_value(*value)
        ),

        Instruction::StateRead { ty, slot } => format!("state.read.{} #{slot}", fmt_type(ty)),
        Instruction::StateWrite { slot, value } => {
            format!("state.write #{slot}, {}", fmt_value(*value))
        }

        Instruction::ReadLocal { local, ty } => format!("read.local.{} l{}", fmt_type(ty), local.0),
        Instruction::WriteLocal { local, value } => {
            format!("write.local l{}, {}", local.0, fmt_value(*value))
        }

        Instruction::Call {
            function,
            args,
            ret_ty,
        } => {
            let args_str = args
                .iter()
                .map(|v| fmt_value(*v))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "call fn{}({args_str}) -> {}",
                function.0,
                fmt_type_owned(ret_ty)
            )
        }

        Instruction::Assert { condition } => format!("assert {}", fmt_value(*condition)),

        Instruction::Poseidon { lhs, rhs } => {
            format!("poseidon {}, {}", fmt_value(*lhs), fmt_value(*rhs))
        }

        Instruction::Emit { event_name, args } => {
            let args_str = args
                .iter()
                .map(|v| fmt_value(*v))
                .collect::<Vec<_>>()
                .join(", ");
            format!("emit {event_name}({args_str})")
        }

        Instruction::ContextRead { kind } => {
            let name = match kind {
                ContextKind::Sender => "sender",
                ContextKind::Nonce => "nonce",
                ContextKind::BlockHeight => "block_height",
            };
            format!("ctx.read.{name}")
        }
    }
}

fn fmt_term(term: &Terminator) -> String {
    match term {
        Terminator::Jump(target) => format!("jump bb{}", target.0),
        Terminator::Branch {
            condition,
            then_block,
            else_block,
        } => format!(
            "br {}, bb{}, bb{}",
            fmt_value(*condition),
            then_block.0,
            else_block.0
        ),
        Terminator::Return(None) => "ret void".to_string(),
        Terminator::Return(Some(v)) => format!("ret {}", fmt_value(*v)),
        Terminator::Unreachable => "unreachable".to_string(),
    }
}
