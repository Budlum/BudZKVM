use crate::ast::*;
use crate::lexer::Token;
use crate::CompileError;
use logos::Logos;

pub struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    _source: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Self {
        let tokens = Token::lexer(source)
            .map(|t| t.unwrap_or(Token::Error))
            .collect();
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

    fn expect(&mut self, expected: Token) -> Result<(), CompileError> {
        let t = self.consume();
        if t != expected {
            return Err(CompileError::ParserError(format!(
                "Expected {:?}, found {:?}",
                expected, t
            )));
        }
        Ok(())
    }

    pub fn parse_contract(&mut self) -> Result<Contract, CompileError> {
        self.expect(Token::Contract)?;
        let name = if let Token::Ident(name) = self.consume() {
            name
        } else {
            return Err(CompileError::ParserError(
                "Expected contract name".to_string(),
            ));
        };

        self.expect(Token::BraceOpen)?;

        let mut functions = Vec::new();
        let mut storage = Vec::new();
        let mut structs = Vec::new();

        while self.peek() != &Token::BraceClose {
            match self.peek() {
                Token::Storage => {
                    self.consume();
                    self.expect(Token::BraceOpen)?;
                    while self.peek() != &Token::BraceClose {
                        let name = if let Token::Ident(name) = self.consume() {
                            name
                        } else {
                            return Err(CompileError::ParserError("Expected name".to_string()));
                        };
                        self.expect(Token::Colon)?;
                        let ty = if let Token::Ident(ty) = self.consume() {
                            if ty == "Map" {
                                self.expect(Token::Lt)?;
                                let k = if let Token::Ident(k) = self.consume() {
                                    k
                                } else {
                                    return Err(CompileError::ParserError(
                                        "Expected map key type".to_string(),
                                    ));
                                };
                                self.expect(Token::Comma)?;
                                let v = if let Token::Ident(v) = self.consume() {
                                    v
                                } else {
                                    return Err(CompileError::ParserError(
                                        "Expected map value type".to_string(),
                                    ));
                                };
                                self.expect(Token::Gt)?;
                                format!("Map<{},{}>", k, v)
                            } else {
                                ty
                            }
                        } else {
                            return Err(CompileError::ParserError("Expected type".to_string()));
                        };
                        self.expect(Token::Comma)?;
                        storage.push(StorageField { name, ty });
                    }
                    self.expect(Token::BraceClose)?;
                }
                Token::Struct => {
                    self.consume();
                    let name = if let Token::Ident(name) = self.consume() {
                        name
                    } else {
                        return Err(CompileError::ParserError("Expected name".to_string()));
                    };
                    self.expect(Token::BraceOpen)?;
                    let mut fields = Vec::new();
                    while self.peek() != &Token::BraceClose {
                        let fname = if let Token::Ident(n) = self.consume() {
                            n
                        } else {
                            return Err(CompileError::ParserError("Expected name".to_string()));
                        };
                        self.expect(Token::Colon)?;
                        let fty = if let Token::Ident(t) = self.consume() {
                            t
                        } else {
                            return Err(CompileError::ParserError("Expected type".to_string()));
                        };
                        self.expect(Token::Comma)?;
                        fields.push(StorageField {
                            name: fname,
                            ty: fty,
                        });
                    }
                    self.expect(Token::BraceClose)?;
                    structs.push(Struct { name, fields });
                }
                _ => {
                    functions.push(self.parse_function()?);
                }
            }
        }
        self.expect(Token::BraceClose)?;

        Ok(Contract {
            name,
            storage,
            structs,
            functions,
        })
    }

    fn parse_function(&mut self) -> Result<Function, CompileError> {
        let is_pub = if self.peek() == &Token::Pub {
            self.consume();
            true
        } else {
            false
        };

        self.expect(Token::Fn)?;
        let name = if let Token::Ident(name) = self.consume() {
            name
        } else {
            return Err(CompileError::ParserError(
                "Expected function name".to_string(),
            ));
        };

        self.expect(Token::ParenOpen)?;
        let mut params = Vec::new();
        while self.peek() != &Token::ParenClose {
            let name = if let Token::Ident(name) = self.consume() {
                name
            } else {
                return Err(CompileError::ParserError("Expected param name".to_string()));
            };
            self.expect(Token::Colon)?;
            let ty = if let Token::Ident(ty) = self.consume() {
                ty
            } else {
                return Err(CompileError::ParserError("Expected param type".to_string()));
            };
            params.push(Param { name, ty });
            if self.peek() == &Token::Comma {
                self.consume();
            }
        }
        self.expect(Token::ParenClose)?;

        let mut return_type = None;
        if self.peek() == &Token::Arrow {
            self.consume();
            if let Token::Ident(ty) = self.consume() {
                return_type = Some(ty);
            } else {
                return Err(CompileError::ParserError("Expected return type".to_string()));
            }
        }

        self.expect(Token::BraceOpen)?;
        let mut body = Vec::new();
        while self.peek() != &Token::BraceClose {
            body.push(self.parse_stmt()?);
        }
        self.expect(Token::BraceClose)?;

        Ok(Function {
            name,
            params,
            return_type,
            body,
            is_pub,
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, CompileError> {
        match self.peek() {
            Token::Let => {
                self.consume();
                let name = if let Token::Ident(name) = self.consume() {
                    name
                } else {
                    return Err(CompileError::ParserError(
                        "Expected identifier after let".to_string(),
                    ));
                };
                self.expect(Token::Assign)?;
                let expr = self.parse_expr()?;
                self.expect(Token::Semicolon)?;
                Ok(Stmt::Let(name, expr))
            }
            Token::Constrain => {
                self.consume();
                self.expect(Token::ParenOpen)?;
                let expr = self.parse_expr()?;
                self.expect(Token::ParenClose)?;
                self.expect(Token::Semicolon)?;
                Ok(Stmt::Constrain(expr))
            }
            Token::Storage => {
                self.consume();
                self.expect(Token::Colon)?;
                self.expect(Token::Colon)?;
                let name = if let Token::Ident(name) = self.consume() {
                    name
                } else {
                    return Err(CompileError::ParserError("Expected name".to_string()));
                };
                self.expect(Token::Assign)?;
                let expr = self.parse_expr()?;
                self.expect(Token::Semicolon)?;
                Ok(Stmt::StorageWrite(name, expr))
            }
            Token::If => {
                self.consume();
                self.expect(Token::ParenOpen)?;
                let cond = self.parse_expr()?;
                self.expect(Token::ParenClose)?;
                self.expect(Token::BraceOpen)?;
                let mut then_branch = Vec::new();
                while self.peek() != &Token::BraceClose {
                    then_branch.push(self.parse_stmt()?);
                }
                self.expect(Token::BraceClose)?;

                let mut else_branch = None;
                if self.peek() == &Token::Else {
                    self.consume();
                    self.expect(Token::BraceOpen)?;
                    let mut eb = Vec::new();
                    while self.peek() != &Token::BraceClose {
                        eb.push(self.parse_stmt()?);
                    }
                    self.expect(Token::BraceClose)?;
                    else_branch = Some(eb);
                }
                Ok(Stmt::If(cond, then_branch, else_branch))
            }
            Token::While => {
                self.consume();
                self.expect(Token::ParenOpen)?;
                let cond = self.parse_expr()?;
                self.expect(Token::ParenClose)?;
                self.expect(Token::BraceOpen)?;
                let mut body = Vec::new();
                while self.peek() != &Token::BraceClose {
                    body.push(self.parse_stmt()?);
                }
                self.expect(Token::BraceClose)?;
                Ok(Stmt::While(cond, body))
            }
            Token::For => {
                self.consume();
                let var = if let Token::Ident(name) = self.consume() {
                    name
                } else {
                    return Err(CompileError::ParserError(
                        "Expected loop variable after for".to_string(),
                    ));
                };
                self.expect(Token::In)?;
                let start = self.parse_expr()?;
                self.expect(Token::DotDot)?;
                let end = self.parse_expr()?;
                self.expect(Token::BraceOpen)?;
                let mut body = Vec::new();
                while self.peek() != &Token::BraceClose {
                    body.push(self.parse_stmt()?);
                }
                self.expect(Token::BraceClose)?;
                Ok(Stmt::For {
                    var,
                    start,
                    end,
                    body,
                })
            }
            Token::Return => {
                self.consume();
                let expr = if self.peek() != &Token::Semicolon {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.expect(Token::Semicolon)?;
                Ok(Stmt::Return(expr))
            }
            Token::Ident(name) if name == "emit" => {
                self.consume();
                let event_name = if let Token::Ident(en) = self.consume() {
                    en
                } else {
                    return Err(CompileError::ParserError("Expected event name".to_string()));
                };
                self.expect(Token::ParenOpen)?;
                let mut args = Vec::new();
                while self.peek() != &Token::ParenClose {
                    args.push(self.parse_expr()?);
                    if self.peek() == &Token::Comma {
                        self.consume();
                    }
                }
                self.expect(Token::ParenClose)?;
                self.expect(Token::Semicolon)?;
                Ok(Stmt::Emit(event_name, args))
            }
            Token::Ident(name) => {
                let name = name.clone();
                self.consume();
                if self.peek() == &Token::BracketOpen {
                    self.consume();
                    let key = self.parse_expr()?;
                    self.expect(Token::BracketClose)?;
                    self.expect(Token::Assign)?;
                    let val = self.parse_expr()?;
                    self.expect(Token::Semicolon)?;
                    Ok(Stmt::MappingWrite(name, key, val))
                } else {
                    self.expect(Token::Assign)?;
                    let expr = self.parse_expr()?;
                    self.expect(Token::Semicolon)?;
                    Ok(Stmt::Assign(name, expr))
                }
            }
            _ => {
                let expr = self.parse_expr()?;
                self.expect(Token::Semicolon)?;
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, CompileError> {
        let mut left = self.parse_arith()?;

        while matches!(
            self.peek(),
            Token::Eq | Token::Neq | Token::Lt | Token::Gt | Token::Lte | Token::Gte
        ) {
            let op = match self.consume() {
                Token::Eq => BinOp::Eq,
                Token::Neq => BinOp::Neq,
                Token::Lt => BinOp::Lt,
                Token::Gt => BinOp::Gt,
                Token::Lte => BinOp::Lte,
                Token::Gte => BinOp::Gte,
                _ => unreachable!(),
            };
            let right = self.parse_arith()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }

        Ok(left)
    }

    fn parse_arith(&mut self) -> Result<Expr, CompileError> {
        let mut left = self.parse_term()?;

        while matches!(self.peek(), Token::Plus | Token::Minus) {
            let op = match self.consume() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => unreachable!(),
            };
            let right = self.parse_term()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }

        Ok(left)
    }

    fn parse_postfix(&mut self) -> Result<Expr, CompileError> {
        let mut expr = self.parse_primary()?;
        while self.peek() == &Token::Dot {
            self.consume();
            let field = if let Token::Ident(f) = self.consume() {
                f
            } else {
                return Err(CompileError::ParserError("Expected field name after dot".to_string()));
            };
            expr = Expr::FieldAccess(Box::new(expr), field);
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expr, CompileError> {
        let mut left = self.parse_postfix()?;

        while matches!(self.peek(), Token::Star | Token::Slash) {
            let op = match self.consume() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                _ => unreachable!(),
            };
            let right = self.parse_postfix()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }

        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expr, CompileError> {
        match self.consume() {
            Token::Int(val) => Ok(Expr::Int(val)),
            Token::Hex(val) => {
                let s = val.strip_prefix("0x").unwrap_or(&val);
                let num = u64::from_str_radix(s, 16).map_err(|e| {
                    CompileError::ParserError(format!("Invalid hex literal {}: {}", val, e))
                })?;
                Ok(Expr::Int(num))
            }
            Token::ParenOpen => {
                let expr = self.parse_expr()?;
                self.expect(Token::ParenClose)?;
                Ok(expr)
            }
            Token::Ident(name) => {
                if name == "poseidon" {
                    self.expect(Token::ParenOpen)?;
                    let mut args = Vec::new();
                    while self.peek() != &Token::ParenClose {
                        args.push(self.parse_expr()?);
                        if self.peek() == &Token::Comma {
                            self.consume();
                        }
                    }
                    self.expect(Token::ParenClose)?;
                    Ok(Expr::Call("poseidon".to_string(), args))
                } else if name == "msg" {
                    self.expect(Token::Colon)?;
                    self.expect(Token::Colon)?;
                    let field = if let Token::Ident(f) = self.consume() {
                        f
                    } else {
                        return Err(CompileError::ParserError("Expected field".to_string()));
                    };
                    self.expect(Token::ParenOpen)?;
                    self.expect(Token::ParenClose)?;
                    Ok(Expr::Call(format!("msg::{}", field), Vec::new()))
                } else if name == "block" {
                    self.expect(Token::Colon)?;
                    self.expect(Token::Colon)?;
                    let field = if let Token::Ident(f) = self.consume() {
                        f
                    } else {
                        return Err(CompileError::ParserError("Expected field".to_string()));
                    };
                    self.expect(Token::ParenOpen)?;
                    self.expect(Token::ParenClose)?;
                    Ok(Expr::Call(format!("block::{}", field), Vec::new()))
                } else if name == "verify_merkle_proof" {
                    self.expect(Token::ParenOpen)?;
                    let root = self.parse_expr()?;
                    self.expect(Token::Comma)?;
                    let leaf = self.parse_expr()?;
                    self.expect(Token::Comma)?;
                    let path = self.parse_expr()?;
                    self.expect(Token::ParenClose)?;
                    Ok(Expr::Call(
                        "verify_merkle_proof".to_string(),
                        vec![root, leaf, path],
                    ))
                } else if self.peek() == &Token::ParenOpen {
                    self.consume();
                    let mut args = Vec::new();
                    while self.peek() != &Token::ParenClose {
                        args.push(self.parse_expr()?);
                        if self.peek() == &Token::Comma {
                            self.consume();
                        }
                    }
                    self.expect(Token::ParenClose)?;
                    Ok(Expr::Call(name, args))
                } else if self.peek() == &Token::BracketOpen {
                    self.consume();
                    let key = self.parse_expr()?;
                    self.expect(Token::BracketClose)?;
                    Ok(Expr::MappingRead(name, Box::new(key)))
                } else if self.peek() == &Token::BraceOpen {
                    self.consume();
                    let mut fields = Vec::new();
                    while self.peek() != &Token::BraceClose {
                        let fname = if let Token::Ident(f) = self.consume() {
                            f
                        } else {
                            return Err(CompileError::ParserError("Expected struct field name".to_string()));
                        };
                        self.expect(Token::Colon)?;
                        let val = self.parse_expr()?;
                        fields.push((fname, val));
                        if self.peek() == &Token::Comma {
                            self.consume();
                        }
                    }
                    self.expect(Token::BraceClose)?;
                    Ok(Expr::StructLiteral(name, fields))
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            Token::Storage => {
                self.expect(Token::Colon)?;
                self.expect(Token::Colon)?;
                let name = if let Token::Ident(name) = self.consume() {
                    name
                } else {
                    return Err(CompileError::ParserError("Expected name".to_string()));
                };
                Ok(Expr::StorageRead(name))
            }
            _ => Err(CompileError::ParserError(
                "Expected primary expression".to_string(),
            )),
        }
    }
}
