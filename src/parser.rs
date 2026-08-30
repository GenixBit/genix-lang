use crate::ast::{BinaryOp, Expr, Function, Param, Program, Stmt, Type, UnaryOp};
use crate::lexer::{Token, TokenKind};

pub fn parse(tokens: Vec<Token>) -> Result<Program, String> {
    Parser { tokens, current: 0 }.parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    fn parse_program(&mut self) -> Result<Program, String> {
        let mut functions = Vec::new();
        while !self.check(&TokenKind::Eof) {
            functions.push(self.parse_function()?);
        }
        self.expect_simple(TokenKind::Eof, "unexpected tokens after program")?;
        if functions.is_empty() {
            return Err("Genix program must define at least fn main()".into());
        }
        Ok(Program { functions })
    }

    fn parse_function(&mut self) -> Result<Function, String> {
        self.expect_simple(TokenKind::Fn, "expected 'fn'")?;
        let name = self.expect_identifier("expected function name after 'fn'")?;
        self.expect_simple(TokenKind::LParen, "expected '(' after function name")?;

        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let param_name = self.expect_identifier("expected parameter name")?;
                self.expect_simple(TokenKind::Colon, "expected ':' after parameter name")?;
                params.push(Param { name: param_name, ty: self.parse_type()? });
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
                if self.check(&TokenKind::RParen) {
                    break;
                }
            }
        }

        self.expect_simple(TokenKind::RParen, "expected ')' after function parameters")?;
        let return_type = if self.matches(&TokenKind::Arrow) {
            self.parse_type()?
        } else {
            Type::Void
        };
        let body = self.parse_block()?;
        Ok(Function { name, params, return_type, body })
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Identifier(name) => match name.as_str() {
                "int" => Ok(Type::Int),
                "float" => Ok(Type::Float),
                "bool" => Ok(Type::Bool),
                "string" => Ok(Type::String),
                "void" => Ok(Type::Void),
                _ => Err(format!("unknown Genix type '{name}' at {}:{}", token.line, token.column)),
            },
            _ => Err(format!("expected type at {}:{}", token.line, token.column)),
        }
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect_simple(TokenKind::LBrace, "expected '{'")?;
        let mut body = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            body.push(self.parse_statement()?);
        }
        self.expect_simple(TokenKind::RBrace, "expected '}' after block")?;
        Ok(body)
    }

    fn parse_statement(&mut self) -> Result<Stmt, String> {
        if self.matches(&TokenKind::Let) {
            return self.parse_binding(false);
        }
        if self.matches(&TokenKind::Mut) {
            return self.parse_binding(true);
        }
        if self.matches(&TokenKind::Return) {
            return self.parse_return();
        }
        if self.matches(&TokenKind::If) {
            return self.parse_if();
        }
        if self.matches(&TokenKind::While) {
            return self.parse_while();
        }
        if self.check(&TokenKind::LBrace) {
            return Ok(Stmt::Block(self.parse_block()?));
        }

        if let TokenKind::Identifier(name) = self.peek().kind.clone() {
            if self.check_next(&TokenKind::Equal) {
                self.advance();
                self.advance();
                let value = self.parse_expression()?;
                self.consume_optional_semicolon();
                return Ok(Stmt::Assign { name, value });
            }
            if name == "print" {
                self.advance();
                self.expect_simple(TokenKind::LParen, "expected '(' after print")?;
                let expr = self.parse_expression()?;
                self.expect_simple(TokenKind::RParen, "expected ')' after print argument")?;
                self.consume_optional_semicolon();
                return Ok(Stmt::Print(expr));
            }
        }

        let expr = self.parse_expression()?;
        self.consume_optional_semicolon();
        if matches!(expr, Expr::Call { .. }) {
            Ok(Stmt::Expr(expr))
        } else {
            Err(self.error_here("only function calls may be used as expression statements"))
        }
    }

    fn parse_binding(&mut self, mutable: bool) -> Result<Stmt, String> {
        let name = self.expect_identifier("expected variable name")?;
        let annotation = if self.matches(&TokenKind::Colon) { Some(self.parse_type()?) } else { None };
        self.expect_simple(TokenKind::Equal, "expected '=' after variable name")?;
        let value = self.parse_expression()?;
        self.consume_optional_semicolon();
        Ok(Stmt::Let { name, value, mutable, annotation })
    }

    fn parse_return(&mut self) -> Result<Stmt, String> {
        if self.check(&TokenKind::Semicolon) {
            self.advance();
            return Ok(Stmt::Return(None));
        }
        if self.check(&TokenKind::RBrace) {
            return Ok(Stmt::Return(None));
        }
        let value = self.parse_expression()?;
        self.consume_optional_semicolon();
        Ok(Stmt::Return(Some(value)))
    }

    fn parse_if(&mut self) -> Result<Stmt, String> {
        let condition = self.parse_expression()?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.matches(&TokenKind::Else) { Some(self.parse_block()?) } else { None };
        Ok(Stmt::If { condition, then_branch, else_branch })
    }

    fn parse_while(&mut self) -> Result<Stmt, String> {
        let condition = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(Stmt::While { condition, body })
    }

    fn parse_expression(&mut self) -> Result<Expr, String> { self.parse_or() }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_and()?;
        while self.matches(&TokenKind::OrOr) {
            let right = self.parse_and()?;
            expr = Expr::Binary { left: Box::new(expr), op: BinaryOp::Or, right: Box::new(right) };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_equality()?;
        while self.matches(&TokenKind::AndAnd) {
            let right = self.parse_equality()?;
            expr = Expr::Binary { left: Box::new(expr), op: BinaryOp::And, right: Box::new(right) };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_comparison()?;
        loop {
            let op = if self.matches(&TokenKind::EqualEqual) {
                Some(BinaryOp::Equal)
            } else if self.matches(&TokenKind::BangEqual) {
                Some(BinaryOp::NotEqual)
            } else { None };
            let Some(op) = op else { break };
            let right = self.parse_comparison()?;
            expr = Expr::Binary { left: Box::new(expr), op, right: Box::new(right) };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_addition()?;
        loop {
            let op = if self.matches(&TokenKind::Less) {
                Some(BinaryOp::Less)
            } else if self.matches(&TokenKind::LessEqual) {
                Some(BinaryOp::LessEqual)
            } else if self.matches(&TokenKind::Greater) {
                Some(BinaryOp::Greater)
            } else if self.matches(&TokenKind::GreaterEqual) {
                Some(BinaryOp::GreaterEqual)
            } else { None };
            let Some(op) = op else { break };
            let right = self.parse_addition()?;
            expr = Expr::Binary { left: Box::new(expr), op, right: Box::new(right) };
        }
        Ok(expr)
    }

    fn parse_addition(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_multiplication()?;
        loop {
            let op = if self.matches(&TokenKind::Plus) {
                Some(BinaryOp::Add)
            } else if self.matches(&TokenKind::Minus) {
                Some(BinaryOp::Subtract)
            } else { None };
            let Some(op) = op else { break };
            let right = self.parse_multiplication()?;
            expr = Expr::Binary { left: Box::new(expr), op, right: Box::new(right) };
        }
        Ok(expr)
    }

    fn parse_multiplication(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_unary()?;
        loop {
            let op = if self.matches(&TokenKind::Star) {
                Some(BinaryOp::Multiply)
            } else if self.matches(&TokenKind::Slash) {
                Some(BinaryOp::Divide)
            } else { None };
            let Some(op) = op else { break };
            let right = self.parse_unary()?;
            expr = Expr::Binary { left: Box::new(expr), op, right: Box::new(right) };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.matches(&TokenKind::Minus) {
            return Ok(Expr::Unary { op: UnaryOp::Negate, expr: Box::new(self.parse_unary()?) });
        }
        if self.matches(&TokenKind::Bang) {
            return Ok(Expr::Unary { op: UnaryOp::Not, expr: Box::new(self.parse_unary()?) });
        }
        self.parse_call()
    }

    fn parse_call(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;
        while self.matches(&TokenKind::LParen) {
            let mut arguments = Vec::new();
            if !self.check(&TokenKind::RParen) {
                loop {
                    arguments.push(self.parse_expression()?);
                    if !self.matches(&TokenKind::Comma) { break; }
                }
            }
            self.expect_simple(TokenKind::RParen, "expected ')' after function arguments")?;
            expr = match expr {
                Expr::Variable(callee) => Expr::Call { callee, arguments },
                _ => return Err(self.error_here("only named functions can be called in Genix v0.1")),
            };
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Integer(value) => Ok(Expr::Integer(value)),
            TokenKind::Float(value) => Ok(Expr::Float(value)),
            TokenKind::String(value) => Ok(Expr::String(value)),
            TokenKind::True => Ok(Expr::Bool(true)),
            TokenKind::False => Ok(Expr::Bool(false)),
            TokenKind::Identifier(name) => {
                let mut qualified = name;
                while self.matches(&TokenKind::Dot) {
                    let segment = self.expect_identifier("expected identifier after '.'")?;
                    qualified.push('.');
                    qualified.push_str(&segment);
                }
                Ok(Expr::Variable(qualified))
            }
            TokenKind::LParen => {
                let expr = self.parse_expression()?;
                self.expect_simple(TokenKind::RParen, "expected ')' after expression")?;
                Ok(expr)
            }
            _ => Err(format!("expected expression at {}:{}", token.line, token.column)),
        }
    }

    fn consume_optional_semicolon(&mut self) {
        if self.check(&TokenKind::Semicolon) { self.advance(); }
    }

    fn expect_identifier(&mut self, message: &str) -> Result<String, String> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Identifier(name) => Ok(name),
            _ => Err(format!("{message} at {}:{}", token.line, token.column)),
        }
    }

    fn expect_simple(&mut self, expected: TokenKind, message: &str) -> Result<(), String> {
        if self.check(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(self.error_here(message))
        }
    }

    fn matches(&mut self, expected: &TokenKind) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else { false }
    }

    fn check(&self, expected: &TokenKind) -> bool {
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(expected)
    }

    fn check_next(&self, expected: &TokenKind) -> bool {
        self.tokens.get(self.current + 1)
            .map(|token| std::mem::discriminant(&token.kind) == std::mem::discriminant(expected))
            .unwrap_or(false)
    }

    fn advance(&mut self) -> &Token {
        if self.current < self.tokens.len() - 1 {
            self.current += 1;
            &self.tokens[self.current - 1]
        } else { &self.tokens[self.current] }
    }

    fn peek(&self) -> &Token { &self.tokens[self.current] }

    fn error_here(&self, message: &str) -> String {
        let token = self.peek();
        format!("{message} at {}:{}", token.line, token.column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    #[test]
    fn parses_multiple_functions_and_typed_parameters() {
        let source = "fn add(a: int, b: int) -> int { return a + b; } fn main() { let result: int = add(2, 3); print(result); }";
        let program = parse(lex(source).unwrap()).unwrap();
        assert_eq!(program.functions.len(), 2);
        assert_eq!(program.functions[0].name, "add");
        assert_eq!(program.functions[0].params.len(), 2);
        assert_eq!(program.functions[0].return_type, Type::Int);
    }

    #[test]
    fn parses_namespaced_call() {
        let source = "fn main() { let result: int = math.add(2, 3); print(result); }";
        let program = parse(lex(source).unwrap()).unwrap();
        match &program.functions[0].body[0] {
            Stmt::Let { value: Expr::Call { callee, .. }, .. } => assert_eq!(callee, "math.add"),
            _ => panic!("expected namespaced call"),
        }
    }
}
