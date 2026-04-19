#[derive(Debug, Clone)]
pub struct Contract {
    pub name: String,
    pub storage: Vec<StorageField>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct StorageField {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Vec<Stmt>,
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let(String, Expr),
    Constrain(Expr),
    Assign(String, Expr),
    StorageWrite(String, Expr),
    MappingWrite(String, Expr, Expr),
    If(Expr, Vec<Stmt>, Option<Vec<Stmt>>),
    Emit(String, Vec<Expr>),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(u64),
    Ident(String),
    StorageRead(String),
    MappingRead(String, Box<Expr>),
    Binary(Box<Expr>, BinOp, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
}
