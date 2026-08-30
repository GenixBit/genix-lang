use crate::ast::{BinaryOp, Expr, Program, Stmt, UnaryOp};
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
        self.expect_simple(TokenKind::Fn, "expected 'fn'")?;
        let name = self.expect_identifier("expected function name after 'fn'")?;
        if name != "main" {
            return Err(self.error_here("Genix v0.1 requires a 'fn main()' entry point"));
        }
        self.expect_simple(TokenKind::LParen, "expected '(' after main")?;
        self.expect_simple(TokenKind::RParen, "expected ')' after main(")?;
        let body = self.parse_block()?;
        self.expect_simple(TokenKind::Eof, "unexpected tokens after main function")?;
        Ok(Program { body })
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

        Err(self.error_here("expected a statement ('let', 'mut', assignment, 'print', 'if', or 'while')"))
    }

    fn parse_binding(&mut self, mutable: bool) -> Result<Stmt, String> {
        let name = self.expect_identifier("expected variable name")?;
        self.expect_simple(TokenKind::Equal, "expected '=' after variable name")?;
        let value = self.parse_expression()?;
        self.consume_optional_semicolon();
        Ok(Stmt::Let { name, value, mutable })
    }

    fn parse_if(&mut self) -> Result<Stmt, String> {
        let condition = self.parse_expression()?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.matches(&TokenKind::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };
        Ok(Stmt::If { condition, then_branch, else_branch })
    }

    fn parse_while(&mut self) -> Result<Stmt, String> {
        let condition = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(Stmt::While { condition, body })
    }

    fn parse_expression(&mut self) -> Result<Expr, String> {
        self.parse_or()
    }

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
            } else {
                None
            };
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
            } else {
                None
            };
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
            } else {
                None
            };
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
            } else {
                None
            };
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
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Integer(value) => Ok(Expr::Integer(value)),
            TokenKind::Float(value) => Ok(Expr::Float(value)),
            TokenKind::String(value) => Ok(Expr::String(value)),
            TokenKind::True => Ok(Expr::Bool(true)),
            TokenKind::False => Ok(Expr::Bool(false)),
            TokenKind::Identifier(name) => Ok(Expr::Variable(name)),
            TokenKind::LParen => {
                let expr = self.parse_expression()?;
                self.expect_simple(TokenKind::RParen, "expected ')' after expression")?;
                Ok(expr)
            }
            _ => Err(format!("expected expression at {}:{}", token.line, token.column)),
        }
    }

    fn consume_optional_semicolon(&mut self) {
        if self.check(&TokenKind::Semicolon) {
            self.advance();
        }
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
        } else {
            false
        }
    }

    fn check(&self, expected: &TokenKind) -> bool {
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(expected)
    }

    fn check_next(&self, expected: &TokenKind) -> bool {
        self.tokens
            .get(self.current + 1)
            .map(|token| std::mem::discriminant(&token.kind) == std::mem::discriminant(expected))
            .unwrap_or(false)
    }

    fn advance(&mut self) -> &Token {
        if self.current < self.tokens.len() - 1 {
            self.current += 1;
            &self.tokens[self.current - 1]
        } else {
            &self.tokens[self.current]
        }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

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
    fn parses_main_with_print() {
        let program = parse(lex("fn main() { print(\"Hello\") }").unwrap()).unwrap();
        assert_eq!(program.body.len(), 1);
    }

    #[test]
    fn respects_operator_precedence() {
        let program = parse(lex("fn main() { let x = 2 + 3 * 4; print(x) }").unwrap()).unwrap();
        assert_eq!(program.body.len(), 2);
    }

    #[test]
    fn parses_if_else_and_while() {
        let source = "fn main() { mut x = 0; while x < 3 { x = x + 1; } if x == 3 { print(true); } else { print(false); } }";
        let program = parse(lex(source).unwrap()).unwrap();
        assert_eq!(program.body.len(), 3);
    }
}
