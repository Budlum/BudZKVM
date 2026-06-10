import re

with open("bud-compiler/src/parser.rs", "r") as f:
    content = f.read()

# 1. Update parse_function for return type
func_orig = """        self.expect(Token::ParenClose)?;

        self.expect(Token::BraceOpen)?;
        let mut body = Vec::new();"""
func_new = """        self.expect(Token::ParenClose)?;

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
        let mut body = Vec::new();"""
content = content.replace(func_orig, func_new)

# 2. Update Function construction
func_build_orig = """        Ok(Function {
            name,
            params,
            body,
            is_pub,
        })"""
func_build_new = """        Ok(Function {
            name,
            params,
            return_type,
            body,
            is_pub,
        })"""
content = content.replace(func_build_orig, func_build_new)

# 3. Add parse_postfix and use it in parse_term
term_orig = """    fn parse_term(&mut self) -> Result<Expr, CompileError> {
        let mut left = self.parse_primary()?;

        while matches!(self.peek(), Token::Star | Token::Slash) {"""
term_new = """    fn parse_postfix(&mut self) -> Result<Expr, CompileError> {
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

        while matches!(self.peek(), Token::Star | Token::Slash) {"""
content = content.replace(term_orig, term_new)

# 4. Also inside parse_term for the right hand side
term_right_orig = """            let right = self.parse_primary()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));"""
term_right_new = """            let right = self.parse_postfix()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));"""
content = content.replace(term_right_orig, term_right_new)

# 5. Update parse_primary for StructLiteral
prim_orig = """                } else if self.peek() == &Token::BracketOpen {
                    self.consume();
                    let key = self.parse_expr()?;
                    self.expect(Token::BracketClose)?;
                    Ok(Expr::MappingRead(name, Box::new(key)))
                } else {
                    Ok(Expr::Ident(name))
                }"""
prim_new = """                } else if self.peek() == &Token::BracketOpen {
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
                }"""
content = content.replace(prim_orig, prim_new)

with open("bud-compiler/src/parser.rs", "w") as f:
    f.write(content)

print("Patch applied.")
