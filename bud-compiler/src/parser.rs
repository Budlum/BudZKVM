use crate::lexer::Token;
use crate::ast::*;
use logos::Logos;

pub struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    _source: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Self {
        let tokens = Token::lexer(source).map(|t| t.unwrap_or(Token::Error)).collect();
        Self {
            tokens,
            pos: 0,
            _source: source,
        }
    }

    fn peek(&self) -> &Token {
        if self.pos < self.tokens.len() {
            &self.tokens[self.pos]
        } else {
            &Token::Error
        }
    }

    fn consume(&mut self) -> Token {
        let t = self.peek().clone();
        self.pos += 1;
        t
    }

    fn expect(&mut self, expected: Token) {
        let t = self.consume();
        if t != expected {
            panic!("Expected {:?}, found {:?}", expected, t);
        }
    }

    pub fn parse_contract(&mut self) -> Contract {
        self.expect(Token::Contract);
        let name = if let Token::Ident(name) = self.consume() {
            name
        } else {
            panic!("Expected contract name");
        };

        self.expect(Token::BraceOpen);
        
        let mut functions = Vec::new();
        let mut storage = Vec::new();

        while self.peek() != &Token::BraceClose {
            match self.peek() {
                Token::Storage => {
                    self.consume();
                    self.expect(Token::BraceOpen);
                    while self.peek() != &Token::BraceClose {
                        let name = if let Token::Ident(name) = self.consume() { name } else { panic!("Expected name") };
                        self.expect(Token::Colon);
                        let ty = if let Token::Ident(ty) = self.consume() { 
                            if ty == "Map" {
                                self.expect(Token::Lt);
                                let k = if let Token::Ident(k) = self.consume() { k } else { panic!("Ex") };
                                self.expect(Token::Comma);
                                let v = if let Token::Ident(v) = self.consume() { v } else { panic!("Ex") };
                                self.expect(Token::Gt);
                                format!("Map<{},{}>", k, v)
                            } else { ty }
                        } else { panic!("Expected type") };
                        self.expect(Token::Comma);
                        storage.push(StorageField { name, ty });
                    }
                    self.expect(Token::BraceClose);
                }
                _ => {
                    functions.push(self.parse_function());
                }
            }
        }
        self.expect(Token::BraceClose);

        Contract {
            name,
            storage,
            functions,
        }
    }

    fn parse_function(&mut self) -> Function {
        let is_pub = if self.peek() == &Token::Pub {
            self.consume();
            true
        } else {
            false
        };

        self.expect(Token::Fn);
        let name = if let Token::Ident(name) = self.consume() {
            name
        } else {
            panic!("Expected function name");
        };

        self.expect(Token::ParenOpen);
        let mut params = Vec::new();
        while self.peek() != &Token::ParenClose {
            let name = if let Token::Ident(name) = self.consume() { name } else { panic!("Expected param name") };
            self.expect(Token::Colon);
            let ty = if let Token::Ident(ty) = self.consume() { ty } else { panic!("Expected param type") };
            params.push(Param { name, ty });
            if self.peek() == &Token::Comma { self.consume(); }
        }
        self.expect(Token::ParenClose);

        self.expect(Token::BraceOpen);
        let mut body = Vec::new();
        while self.peek() != &Token::BraceClose {
            body.push(self.parse_stmt());
        }
        self.expect(Token::BraceClose);

        Function {
            name,
            params,
            body,
            is_pub,
        }
    }

    fn parse_stmt(&mut self) -> Stmt {
        match self.peek() {
            Token::Let => {
                self.consume();
                let name = if let Token::Ident(name) = self.consume() {
                    name
                } else {
                    panic!("Expected identifier after let");
                };
                self.expect(Token::Assign);
                let expr = self.parse_expr();
                self.expect(Token::Semicolon);
                Stmt::Let(name, expr)
            }
            Token::Constrain => {
                self.consume();
                self.expect(Token::ParenOpen);
                let expr = self.parse_expr();
                self.expect(Token::ParenClose);
                self.expect(Token::Semicolon);
                Stmt::Constrain(expr)
            }
            Token::Storage => {
                self.consume();
                self.expect(Token::Colon);
                self.expect(Token::Colon);
                let name = if let Token::Ident(name) = self.consume() { name } else { panic!("Expected name") };
                self.expect(Token::Assign);
                let expr = self.parse_expr();
                self.expect(Token::Semicolon);
                Stmt::StorageWrite(name, expr)
            }
            Token::If => {
                self.consume();
                self.expect(Token::ParenOpen);
                let cond = self.parse_expr();
                self.expect(Token::ParenClose);
                self.expect(Token::BraceOpen);
                let mut then_branch = Vec::new();
                while self.peek() != &Token::BraceClose {
                    then_branch.push(self.parse_stmt());
                }
                self.expect(Token::BraceClose);
                
                let mut else_branch = None;
                if self.peek() == &Token::Else {
                    self.consume();
                    self.expect(Token::BraceOpen);
                    let mut eb = Vec::new();
                    while self.peek() != &Token::BraceClose {
                        eb.push(self.parse_stmt());
                    }
                    self.expect(Token::BraceClose);
                    else_branch = Some(eb);
                }
                Stmt::If(cond, then_branch, else_branch)
            }
            Token::Ident(name) if name == "emit" => {
                self.consume();
                let event_name = if let Token::Ident(en) = self.consume() { en } else { panic!("Expected event name") };
                self.expect(Token::ParenOpen);
                let mut args = Vec::new();
                while self.peek() != &Token::ParenClose {
                    args.push(self.parse_expr());
                    if self.peek() == &Token::Comma { self.consume(); }
                }
                self.expect(Token::ParenClose);
                self.expect(Token::Semicolon);
                Stmt::Emit(event_name, args)
            }
            Token::Ident(name) => {
                let name = name.clone();
                self.consume();
                if self.peek() == &Token::BracketOpen {
                    self.consume();
                    let key = self.parse_expr();
                    self.expect(Token::BracketClose);
                    self.expect(Token::Assign);
                    let val = self.parse_expr();
                    self.expect(Token::Semicolon);
                    Stmt::MappingWrite(name, key, val)
                } else {
                    self.expect(Token::Assign);
                    let expr = self.parse_expr();
                    self.expect(Token::Semicolon);
                    Stmt::Assign(name, expr)
                }
            }
            _ => {
                let expr = self.parse_expr();
                self.expect(Token::Semicolon);
                Stmt::Expr(expr)
            }
        }
    }

    fn parse_expr(&mut self) -> Expr {
        let mut left = self.parse_arith();
        
        while matches!(self.peek(), Token::Eq | Token::Neq | Token::Lt | Token::Gt | Token::Lte | Token::Gte) {
            let op = match self.consume() {
                Token::Eq => BinOp::Eq,
                Token::Neq => BinOp::Neq,
                Token::Lt => BinOp::Lt,
                Token::Gt => BinOp::Gt,
                Token::Lte => BinOp::Lte,
                Token::Gte => BinOp::Gte,
                _ => unreachable!(),
            };
            let right = self.parse_arith();
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }

        left
    }

    fn parse_arith(&mut self) -> Expr {
        let mut left = self.parse_primary();
        
        while matches!(self.peek(), Token::Plus | Token::Minus | Token::Star | Token::Slash) {
            let op = match self.consume() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                _ => unreachable!(),
            };
            let right = self.parse_primary();
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }

        left
    }

    fn parse_primary(&mut self) -> Expr {
        match self.consume() {
            Token::Int(val) => Expr::Int(val),
            Token::Ident(name) => {
                if name == "poseidon" {
                    self.expect(Token::ParenOpen);
                    let mut args = Vec::new();
                    while self.peek() != &Token::ParenClose {
                        args.push(self.parse_expr());
                        if self.peek() == &Token::Comma { self.consume(); }
                    }
                    self.expect(Token::ParenClose);
                    Expr::Call("poseidon".to_string(), args)
                } else if name == "msg" {
                    self.expect(Token::Colon);
                    self.expect(Token::Colon);
                    let field = if let Token::Ident(f) = self.consume() { f } else { panic!("Expected field") };
                    self.expect(Token::ParenOpen);
                    self.expect(Token::ParenClose);
                    Expr::Call(format!("msg::{}", field), Vec::new())
                } else if name == "block" {
                    self.expect(Token::Colon);
                    self.expect(Token::Colon);
                    let field = if let Token::Ident(f) = self.consume() { f } else { panic!("Expected field") };
                    self.expect(Token::ParenOpen);
                    self.expect(Token::ParenClose);
                    Expr::Call(format!("block::{}", field), Vec::new())
                } else if name == "verify_merkle_proof" {
                    self.expect(Token::ParenOpen);
                    let root = self.parse_expr();
                    self.expect(Token::Comma);
                    let leaf = self.parse_expr();
                    self.expect(Token::Comma);
                    let path = self.parse_expr();
                    self.expect(Token::ParenClose);
                    Expr::Call("verify_merkle_proof".to_string(), vec![root, leaf, path])
                } else if self.peek() == &Token::BracketOpen {
                    self.consume();
                    let key = self.parse_expr();
                    self.expect(Token::BracketClose);
                    Expr::MappingRead(name, Box::new(key))
                } else {
                    Expr::Ident(name)
                }
            }
            Token::Storage => {
                self.expect(Token::Colon);
                self.expect(Token::Colon);
                let name = if let Token::Ident(name) = self.consume() { name } else { panic!("Expected name") };
                Expr::StorageRead(name)
            }
            _ => panic!("Expected primary expression"),
        }
    }
}
