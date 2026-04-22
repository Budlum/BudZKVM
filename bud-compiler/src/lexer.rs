use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
pub enum Token {
    #[token("contract")]
    Contract,
    #[token("fn")]
    Fn,
    #[token("let")]
    Let,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("match")]
    Match,
    #[token("storage")]
    Storage,
    #[token("witness")]
    Witness,
    #[token("constrain")]
    Constrain,
    #[token("pub")]
    Pub,
    #[token("while")]
    While,
    #[token("for")]
    For,
    #[token("return")]
    Return,
    #[token("struct")]
    Struct,

    #[token("{")]
    BraceOpen,
    #[token("}")]
    BraceClose,
    #[token("(")]
    ParenOpen,
    #[token(")")]
    ParenClose,
    #[token("[")]
    BracketOpen,
    #[token("]")]
    BracketClose,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token(";")]
    Semicolon,
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,

    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("==")]
    Eq,
    #[token("!=")]
    Neq,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("<=")]
    Lte,
    #[token(">=")]
    Gte,
    #[token("=")]
    Assign,

    #[regex("[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    #[regex("[0-9]+", |lex| lex.slice().parse::<u64>().ok())]
    Int(u64),

    #[regex("0x[0-9a-fA-F]+", |lex| lex.slice().to_string())]
    Hex(String),

    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}
