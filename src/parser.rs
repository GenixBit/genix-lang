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
        self.expect_simple(TokenKind::LBrace, "expected '{' before main body")?;

        let mut body = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            body.push(self.parse_statement()?);
        }

        self.expect_simple(TokenKind::RBrace, "expected '}' after main body")?;
        self.expect_simple(TokenKind::Eof, "unexpected tokens after main function")?;
        Ok(Program { body })
    }

    fn parse_statement(&mut self) -> Result<Stmt, String> {
        if self.matches(&TokenKind::Let) {
            return self.parse_let();
        }

        if let TokenKind::Identifier(name) = self.peek().kind.clone() {
            if name == "print" {
                self.advance();
                self.expect_simple(TokenKind::LParen, "expected '(' after print")?;
                let expr = self.parse_expression()?;
                self.expect_simple(TokenKind::RParen, "expected ')' after print argument")?;
                self.consume_optional_semicolon();
                return Ok(Stmt::Print(expr));
            }
        }

        Err(self.error_here("expected a statement ('let' or 'print')"))
    }

    fn parse_let(&mut self) -> Result<Stmt, String> {
        let name = self.expect_identifier("expected variable name after 'let'")?;
        self.expect_simple(TokenKind::Equal, "expected '=' after variable name")?;
        let value = self.parse_expression()?;
        self.consume_optional_semicolon();
        Ok(Stmt::Let { name, value })
    }

    fn parse_expression(&mut self) -> Result<Expr, String> {
        self.parse_addition()
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
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
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
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.matches(&TokenKind::Minus) {
            return Ok(Expr::Unary {
                op: UnaryOp::Negate,
                expr: Box::new(self.parse_unary()?),
            });
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
            _ => Err(format!(
                "expected expression at {}:{}",
                token.line, token.column
            )),
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
}
