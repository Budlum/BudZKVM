//! # BudIR — Canonical, Target-Independent Intermediate Representation
//!
//! BudIR ne BudL sözdizimine, ne fiziksel VM register'larına, ne PC/jump
//! offset'lerine, ne STARK trace kolonlarına, ne de Plonky3 ayrıntılarına
//! bağımlıdır. Herhangi bir frontend (bugün BudL, yarın AI-native) aynı IR'a
//! lower edilebilir; herhangi bir backend (BudVM ISA, WASM, EVM …) bu IR'dan
//! kod üretebilir.
//!
//! ## Hiyerarşi
//! ```text
//! IrProgram
//!  └── IrFunction*
//!       └── BasicBlock*
//!            ├── InstrNode*  (instruction + optional result ValueId)
//!            └── Terminator  (exactly one per block)
//! ```
//!
//! ## SSA Durumu
//! Her hesaplamanın sonucu benzersiz bir [`ValueId`] üretir (single-assignment).
//! Değişebilir yerel değişkenler (BudL `let` + atama) [`LocalId`] slotları
//! üzerinden modellenir (`ReadLocal` / `WriteLocal`). Bu, tam akademik SSA'nın
//! öncülü olan "pre-SSA" / "mem2reg öncesi" stildir. İleride bir `mem2reg` geçişi
//! bu slotları gerçek SSA phi node'larına dönüştürebilir.

mod display;
pub mod lower;
pub mod verify;

use std::collections::HashMap;

// ─── Tanımlayıcılar ───────────────────────────────────────────────────────

/// Tek bir hesaplamayı temsil eden, değiştirilemez SSA değer tanıtıcısı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ValueId(pub(crate) u32);

/// Bir fonksiyon içindeki temel bloğu tanımlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub(crate) u32);

/// [`IrProgram`] içindeki bir fonksiyonu tanımlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FunctionId(pub(crate) u32);

/// Değişebilir yerel değişken slotu (pre-SSA).
///
/// Aynı `LocalId` birden fazla kez yazılabilir (tam SSA değil).
/// Türü [`IrFunction::locals`] listesinde tutulur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalId(pub(crate) u32);

impl ValueId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
    #[cfg(test)]
    pub fn new(v: u32) -> Self {
        Self(v)
    }
}
impl BlockId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
    #[cfg(test)]
    pub fn new(v: u32) -> Self {
        Self(v)
    }
}
impl FunctionId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}
impl LocalId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
    #[cfg(test)]
    pub fn new(v: u32) -> Self {
        Self(v)
    }
}

// ─── Tipler ───────────────────────────────────────────────────────────────

/// Kanonik IR tipi. `sema::Type::Unknown` lowering hatasına dönüşür.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrType {
    U64,
    Bool,
    Field,
    Struct(String),
    Void,
}

// ─── Etki Modeli ──────────────────────────────────────────────────────────

/// Instruction'ın kaba yan-etki sınıfı.
///
/// Capability sistemi değildir. Optimizatörler ve AI frontend'ler bu bilgiyi
/// ISA/backend detaylarını bilmeden sorgulayabilir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Gözlemlenebilir yan etkisi yok.
    Pure,
    /// Heap belleğinden okur.
    MemoryRead,
    /// Heap belleğine yazar.
    MemoryWrite,
    /// Kalıcı contract storage'ından okur.
    StateRead,
    /// Kalıcı contract storage'ına yazar.
    StateWrite,
    /// Yürütme bağlamını okur (sender, block height, …).
    ContextRead,
    /// Off-chain log event'i yayınlar.
    Event,
    /// Başka fonksiyonu çağırır; herhangi bir yan etkiye sahip olabilir.
    Call,
}

// ─── Bağlam Alanları ──────────────────────────────────────────────────────

/// Yürütme bağlamı alanı için anlamsal tanımlayıcı.
///
/// ISA'daki `Syscall 1/2/3` sayısal eşlemesi kasıtlı olarak backend'e bırakılır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextKind {
    /// `msg::sender()` — işlemi başlatan hesap.
    Sender,
    /// `msg::nonce()` — işlem nonce'u.
    Nonce,
    /// `block::number()` — mevcut blok yüksekliği.
    BlockHeight,
}

// ─── Instruction ──────────────────────────────────────────────────────────

/// Tek bir BudIR instruction'ı.
///
/// Sonuç [`ValueId`]'i burada değil, [`InstrNode::result`]'da tutulur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    // Sabitler
    Const {
        ty: IrType,
        value: u64,
    },

    // Aritmetik
    Add {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Sub {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Mul {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Div {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },

    // Karşılaştırma (sonuç tipi daima Bool)
    IrEq {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    IrNe {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Lt {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Le {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Gt {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },
    Ge {
        ty: IrType,
        lhs: ValueId,
        rhs: ValueId,
    },

    // Heap belleği (struct alanları)
    Load {
        ty: IrType,
        base: ValueId,
        offset: i64,
    },
    Store {
        base: ValueId,
        offset: i64,
        value: ValueId,
    },

    // Contract kalıcı storage
    StateRead {
        ty: IrType,
        slot: i32,
    },
    StateWrite {
        slot: i32,
        value: ValueId,
    },

    // Değişebilir yerel değişkenler (pre-SSA)
    ReadLocal {
        local: LocalId,
        ty: IrType,
    },
    WriteLocal {
        local: LocalId,
        value: ValueId,
    },

    // Fonksiyon çağrısı
    Call {
        function: FunctionId,
        args: Vec<ValueId>,
        ret_ty: IrType,
    },

    // ZK / etki alanına özgü
    Assert {
        condition: ValueId,
    },
    Poseidon {
        lhs: ValueId,
        rhs: ValueId,
    },
    Emit {
        event_name: String,
        args: Vec<ValueId>,
    },
    ContextRead {
        kind: ContextKind,
    },
}

impl Instruction {
    /// Bu instruction'ın ürettiği değerin IR tipi; sonuç yoksa `None`.
    pub fn result_type(&self) -> Option<IrType> {
        match self {
            Instruction::Const { ty, .. } => Some(ty.clone()),
            Instruction::Add { ty, .. }
            | Instruction::Sub { ty, .. }
            | Instruction::Mul { ty, .. }
            | Instruction::Div { ty, .. } => Some(ty.clone()),
            Instruction::IrEq { .. }
            | Instruction::IrNe { .. }
            | Instruction::Lt { .. }
            | Instruction::Le { .. }
            | Instruction::Gt { .. }
            | Instruction::Ge { .. } => Some(IrType::Bool),
            Instruction::Load { ty, .. } => Some(ty.clone()),
            Instruction::Store { .. } => None,
            Instruction::StateRead { ty, .. } => Some(ty.clone()),
            Instruction::StateWrite { .. } => None,
            Instruction::ReadLocal { ty, .. } => Some(ty.clone()),
            Instruction::WriteLocal { .. } => None,
            Instruction::Call { ret_ty, .. } => {
                if *ret_ty == IrType::Void {
                    None
                } else {
                    Some(ret_ty.clone())
                }
            }
            Instruction::Assert { .. } => None,
            Instruction::Poseidon { .. } => Some(IrType::U64),
            Instruction::Emit { .. } => None,
            Instruction::ContextRead { .. } => Some(IrType::U64),
        }
    }

    /// Bu instruction'ın kaba yan-etki sınıfı.
    pub fn effect(&self) -> Effect {
        match self {
            Instruction::Const { .. }
            | Instruction::Add { .. }
            | Instruction::Sub { .. }
            | Instruction::Mul { .. }
            | Instruction::Div { .. }
            | Instruction::IrEq { .. }
            | Instruction::IrNe { .. }
            | Instruction::Lt { .. }
            | Instruction::Le { .. }
            | Instruction::Gt { .. }
            | Instruction::Ge { .. }
            | Instruction::Poseidon { .. }
            | Instruction::Assert { .. }
            | Instruction::ReadLocal { .. } => Effect::Pure,
            Instruction::Load { .. } => Effect::MemoryRead,
            Instruction::Store { .. } | Instruction::WriteLocal { .. } => Effect::MemoryWrite,
            Instruction::StateRead { .. } => Effect::StateRead,
            Instruction::StateWrite { .. } => Effect::StateWrite,
            Instruction::ContextRead { .. } => Effect::ContextRead,
            Instruction::Emit { .. } => Effect::Event,
            Instruction::Call { .. } => Effect::Call,
        }
    }
}

// ─── Terminator ───────────────────────────────────────────────────────────

/// Bir BasicBlock'u kapatan control-flow instruction'ı.
/// Her blok tam olarak bir Terminator ile bitmek zorundadır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    /// Koşulsuz dal.
    Jump(BlockId),
    /// Koşullu dal.
    Branch {
        condition: ValueId,
        then_block: BlockId,
        else_block: BlockId,
    },
    /// Fonksiyondan dön; opsiyonel değer ile.
    Return(Option<ValueId>),
    /// Statik olarak erişilemeyen yol.
    Unreachable,
}

// ─── BasicBlock ───────────────────────────────────────────────────────────

/// Instruction + opsiyonel sonuç çifti.
#[derive(Debug, Clone)]
pub struct InstrNode {
    /// SSA değeri; void instruction'lar için `None`.
    pub result: Option<ValueId>,
    pub instr: Instruction,
}

/// Tek giriş noktası ve tek Terminator ile biten instruction dizisi.
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BlockId,
    pub instrs: Vec<InstrNode>,
    /// IR inşaası sırasında `None`; verifier tamamlanmış IR'da `None` görürse hata verir.
    pub terminator: Option<Terminator>,
}

// ─── Function ─────────────────────────────────────────────────────────────

/// Bir IR fonksiyonu. `blocks[0]` entry bloğudur.
#[derive(Debug, Clone)]
pub struct IrFunction {
    pub id: FunctionId,
    pub name: String,
    /// Parametre listesi: (value_id, tür).
    pub params: Vec<(ValueId, IrType)>,
    pub ret_ty: IrType,
    /// Temel bloklar, [`BlockId`] sırasına göre tutulur (deterministik çıktı).
    pub blocks: Vec<BasicBlock>,
    /// Değişebilir yerel slot türleri, [`LocalId`] ile indekslenir.
    pub locals: Vec<IrType>,
}

// ─── Program ──────────────────────────────────────────────────────────────

/// Tek bir `.bud` contract'ından üretilen üst düzey IR artefaktı.
#[derive(Debug, Clone, Default)]
pub struct IrProgram {
    pub functions: Vec<IrFunction>,
    /// İsim → ID; çağrı çözümleme ve görüntüleme için.
    pub function_names: HashMap<String, FunctionId>,
}
