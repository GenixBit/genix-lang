use crate::ast::{
    BinaryOp, Expr, Function, MatchArm, Param, Pattern, Program, Stmt, StmtKind, Type, UnaryOp,
};
use crate::diagnostics::{Diagnostic, Span};
use crate::lexer::{Token, TokenKind};

pub fn parse(tokens: Vec<Token>) -> Result<Program, Diagnostic> {
    parse_named(tokens, "<memory>")
}

pub fn parse_named(tokens: Vec<Token>, source_name: impl Into<String>) -> Result<Program, Diagnostic> {
    Parser {
        tokens,
        current: 0,
        source_name: source_name.into(),
    }
    .parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    current: usize,
    source_name: String,
}

impl Parser {
    fn parse_program(&mut self) -> Result<Program, Diagnostic> {
        let mut functions = Vec::new();
        while !self.check(&TokenKind::Eof) {
            functions.push(self.parse_function()?);
        }
        self.expect_simple(TokenKind::Eof, "unexpected tokens after program")?;
        if functions.is_empty() {
            return Err(
                self.error_here("Genix program must define at least fn main()")
                    .with_help("add `fn main() { ... }` as the program entry point"),
            );
        }
        Ok(Program { functions })
    }

    fn parse_function(&mut self) -> Result<Function, Diagnostic> {
        let start = self.peek().clone();
        self.expect_simple(TokenKind::Fn, "expected 'fn'")?;
        let name = self.expect_identifier("expected function name after 'fn'")?;
        self.expect_simple(TokenKind::LParen, "expected '(' after function name")?;

        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let param_name = self.expect_identifier("expected parameter name")?;
                self.expect_simple(TokenKind::Colon, "expected ':' after parameter name")?;
                params.push(Param {
                    name: param_name,
                    ty: self.parse_type()?,
                });
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
        let span = self.span_from(&start);
        Ok(Function {
            name,
            params,
            return_type,
            body,
            source_name: self.source_name.clone(),
            span,
        })
    }

    fn parse_type(&mut self) -> Result<Type, Diagnostic> {
        let token = self.advance().clone();
        let TokenKind::Identifier(name) = token.kind else {
            return Err(
                self.error_at(&token, "E0101", "expected a Genix type")
                    .with_label("type expected here"),
            );
        };

        match name.as_str() {
            "int" => Ok(Type::Int),
            "float" => Ok(Type::Float),
            "bool" => Ok(Type::Bool),
            "string" => Ok(Type::String),
            "void" => Ok(Type::Void),
            "Option" => {
                self.expect_simple(TokenKind::Less, "expected '<' after Option")?;
                let inner = self.parse_type()?;
                self.expect_simple(TokenKind::Greater, "expected '>' after Option type")?;
                Type::option(inner).ok_or_else(|| {
                    self.error_at(
                        &token,
                        "E0102",
                        format!("Option<{inner}> is not supported in Genix v0.1"),
                    )
                    .with_label("unsupported Option payload")
                    .with_help("Option currently supports int, float, bool, and string payloads")
                })
            }
            "Result" => {
                self.expect_simple(TokenKind::Less, "expected '<' after Result")?;
                let ok = self.parse_type()?;
                self.expect_simple(TokenKind::Comma, "expected ',' between Result types")?;
                let error = self.parse_type()?;
                self.expect_simple(TokenKind::Greater, "expected '>' after Result types")?;
                Type::result(ok, error).ok_or_else(|| {
                    self.error_at(
                        &token,
                        "E0102",
                        format!("Result<{ok},{error}> is not supported in Genix v0.1"),
                    )
                    .with_label("unsupported Result shape")
                    .with_help("Result currently supports primitive success values and string errors")
                })
            }
            _ => Err(
                self.error_at(&token, "E0101", format!("unknown Genix type '{name}'"))
                    .with_label("unknown type")
                    .with_help("use int, float, bool, string, Option<T>, or Result<T,string>"),
            ),
        }
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, Diagnostic> {
        self.expect_simple(TokenKind::LBrace, "expected '{'")?;
        let mut body = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            body.push(self.parse_statement()?);
        }
        self.expect_simple(TokenKind::RBrace, "expected '}' after block")?;
        Ok(body)
    }

    fn parse_statement(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.peek().clone();
        let kind = if self.matches(&TokenKind::Let) {
            self.parse_binding(false)?
        } else if self.matches(&TokenKind::Mut) {
            self.parse_binding(true)?
        } else if self.matches(&TokenKind::Return) {
            self.parse_return()?
        } else if self.matches(&TokenKind::If) {
            self.parse_if()?
        } else if self.matches(&TokenKind::While) {
            self.parse_while()?
        } else if self.matches(&TokenKind::Match) {
            self.parse_match()?
        } else if self.check(&TokenKind::LBrace) {
            StmtKind::Block(self.parse_block()?)
        } else if let TokenKind::Identifier(name) = self.peek().kind.clone() {
            if self.check_next(&TokenKind::Equal) {
                self.advance();
                self.advance();
                let value = self.parse_expression()?;
                self.consume_optional_semicolon();
                StmtKind::Assign { name, value }
            } else if name == "print" {
                self.advance();
                self.expect_simple(TokenKind::LParen, "expected '(' after print")?;
                let expr = self.parse_expression()?;
                self.expect_simple(TokenKind::RParen, "expected ')' after print argument")?;
                self.consume_optional_semicolon();
                StmtKind::Print(expr)
            } else {
                self.parse_expression_statement()?
            }
        } else {
            self.parse_expression_statement()?
        };

        Ok(Stmt::new(kind, self.span_from(&start)))
    }

    fn parse_expression_statement(&mut self) -> Result<StmtKind, Diagnostic> {
        let expr = self.parse_expression()?;
        self.consume_optional_semicolon();
        if matches!(expr, Expr::Call { .. } | Expr::Try(_)) {
            Ok(StmtKind::Expr(expr))
        } else {
            Err(
                self.error_here("only function calls may be used as expression statements")
                    .with_help("assign the value to a variable, return it, or pass it to a function"),
            )
        }
    }

    fn parse_binding(&mut self, mutable: bool) -> Result<StmtKind, Diagnostic> {
        let name = self.expect_identifier("expected variable name")?;
        let annotation = if self.matches(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect_simple(TokenKind::Equal, "expected '=' after variable name")?;
        let value = self.parse_expression()?;
        self.consume_optional_semicolon();
        Ok(StmtKind::Let {
            name,
            value,
            mutable,
            annotation,
        })
    }

    fn parse_return(&mut self) -> Result<StmtKind, Diagnostic> {
        if self.check(&TokenKind::Semicolon) {
            self.advance();
            return Ok(StmtKind::Return(None));
        }
        if self.check(&TokenKind::RBrace) {
            return Ok(StmtKind::Return(None));
        }
        let value = self.parse_expression()?;
        self.consume_optional_semicolon();
        Ok(StmtKind::Return(Some(value)))
    }

    fn parse_if(&mut self) -> Result<StmtKind, Diagnostic> {
        let condition = self.parse_expression()?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.matches(&TokenKind::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };
        Ok(StmtKind::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn parse_while(&mut self) -> Result<StmtKind, Diagnostic> {
        let condition = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(StmtKind::While { condition, body })
    }

    fn parse_match(&mut self) -> Result<StmtKind, Diagnostic> {
        let value = self.parse_expression()?;
        self.expect_simple(TokenKind::LBrace, "expected '{' after match value")?;
        let mut arms = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            let pattern = self.parse_pattern()?;
            self.expect_simple(TokenKind::FatArrow, "expected '=>' after match pattern")?;
            let body = self.parse_block()?;
            self.matches(&TokenKind::Comma);
            arms.push(MatchArm { pattern, body });
        }
        self.expect_simple(TokenKind::RBrace, "expected '}' after match arms")?;
        if arms.is_empty() {
            return Err(
                self.error_here("match requires at least one arm")
                    .with_help("add Some/None or Ok/Err match arms"),
            );
        }
        Ok(StmtKind::Match { value, arms })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, Diagnostic> {
        let token = self.advance().clone();
        let TokenKind::Identifier(name) = token.kind else {
            return Err(
                self.error_at(&token, "E0103", "expected a match pattern")
                    .with_label("pattern expected here"),
            );
        };
        match name.as_str() {
            "Some" => {
                self.expect_simple(TokenKind::LParen, "expected '(' after Some")?;
                let binding = self.expect_identifier("expected binding name in Some pattern")?;
                self.expect_simple(TokenKind::RParen, "expected ')' after Some binding")?;
                Ok(Pattern::Some(binding))
            }
            "None" => Ok(Pattern::None),
            "Ok" => {
                self.expect_simple(TokenKind::LParen, "expected '(' after Ok")?;
                let binding = self.expect_identifier("expected binding name in Ok pattern")?;
                self.expect_simple(TokenKind::RParen, "expected ')' after Ok binding")?;
                Ok(Pattern::Ok(binding))
            }
            "Err" => {
                self.expect_simple(TokenKind::LParen, "expected '(' after Err")?;
                let binding = self.expect_identifier("expected binding name in Err pattern")?;
                self.expect_simple(TokenKind::RParen, "expected ')' after Err binding")?;
                Ok(Pattern::Err(binding))
            }
            _ => Err(
                self.error_at(&token, "E0103", format!("unknown match pattern '{name}'"))
                    .with_label("invalid pattern")
                    .with_help("use Some(name), None, Ok(name), or Err(name)"),
            ),
        }
    }

    fn parse_expression(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_and()?;
        while self.matches(&TokenKind::OrOr) {
            let right = self.parse_and()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::Or,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_equality()?;
        while self.matches(&TokenKind::AndAnd) {
            let right = self.parse_equality()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::And,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, Diagnostic> {
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
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, Diagnostic> {
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
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_addition(&mut self) -> Result<Expr, Diagnostic> {
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

    fn parse_multiplication(&mut self) -> Result<Expr, Diagnostic> {
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

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.matches(&TokenKind::Minus) {
            return Ok(Expr::Unary {
                op: UnaryOp::Negate,
                expr: Box::new(self.parse_unary()?),
            });
        }
        if self.matches(&TokenKind::Bang) {
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(self.parse_unary()?),
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.matches(&TokenKind::LParen) {
                let call_token = self.previous().clone();
                let mut arguments = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        arguments.push(self.parse_expression()?);
                        if !self.matches(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect_simple(TokenKind::RParen, "expected ')' after function arguments")?;
                expr = match expr {
                    Expr::Variable(callee) if callee == "Some" => {
                        if arguments.len() != 1 {
                            return Err(
                                self.error_at(&call_token, "E0104", "Some(...) requires exactly one value")
                                    .with_help("pass one value, for example Some(value)"),
                            );
                        }
                        Expr::Some(Box::new(arguments.remove(0)))
                    }
                    Expr::Variable(callee) if callee == "Ok" => {
                        if arguments.len() != 1 {
                            return Err(
                                self.error_at(&call_token, "E0104", "Ok(...) requires exactly one value")
                                    .with_help("pass one success value, for example Ok(value)"),
                            );
                        }
                        Expr::Ok(Box::new(arguments.remove(0)))
                    }
                    Expr::Variable(callee) if callee == "Err" => {
                        if arguments.len() != 1 {
                            return Err(
                                self.error_at(&call_token, "E0104", "Err(...) requires exactly one error value")
                                    .with_help("pass one string error, for example Err(\"message\")"),
                            );
                        }
                        Expr::Err(Box::new(arguments.remove(0)))
                    }
                    Expr::Variable(callee) if callee == "None" => {
                        return Err(
                            self.error_at(&call_token, "E0104", "None does not take arguments")
                                .with_help("write None without parentheses"),
                        )
                    }
                    Expr::Variable(callee) => Expr::Call { callee, arguments },
                    _ => {
                        return Err(
                            self.error_here("only named functions can be called in Genix v0.1")
                                .with_help("call a function by name, such as load() or fs.read_text(...)"),
                        )
                    }
                };
            } else if self.matches(&TokenKind::Question) {
                expr = Expr::Try(Box::new(expr));
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Integer(value) => Ok(Expr::Integer(value)),
            TokenKind::Float(value) => Ok(Expr::Float(value)),
            TokenKind::String(value) => Ok(Expr::String(value)),
            TokenKind::True => Ok(Expr::Bool(true)),
            TokenKind::False => Ok(Expr::Bool(false)),
            TokenKind::Identifier(name) => {
                if name == "None" {
                    return Ok(Expr::None);
                }
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
            _ => Err(
                self.error_at(&token, "E0100", "expected expression")
                    .with_label("expression expected here"),
            ),
        }
    }

    fn consume_optional_semicolon(&mut self) {
        if self.check(&TokenKind::Semicolon) {
            self.advance();
        }
    }

    fn expect_identifier(&mut self, message: &str) -> Result<String, Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Identifier(name) => Ok(name),
            _ => Err(self.error_at(&token, "E0100", message).with_label("identifier expected here")),
        }
    }

    fn expect_simple(&mut self, expected: TokenKind, message: &str) -> Result<(), Diagnostic> {
        if self.check(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(self.error_here(message).with_label("syntax error"))
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

    fn previous(&self) -> &Token {
        if self.current == 0 {
            &self.tokens[0]
        } else {
            &self.tokens[self.current - 1]
        }
    }

    fn span_from(&self, start: &Token) -> Span {
        let end = self.previous();
        Span::between(
            start.line,
            start.column,
            end.line,
            end.column + end.width.saturating_sub(1),
        )
    }

    fn error_here(&self, message: impl Into<String>) -> Diagnostic {
        let token = self.peek();
        self.error_at(token, "E0100", message)
    }

    fn error_at(&self, token: &Token, code: &'static str, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(code, message)
            .with_location(self.source_name.clone(), token.span())
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
        assert!(program.functions[0].span.end_column >= program.functions[0].span.column);
    }

    #[test]
    fn preserves_source_name() {
        let program = parse_named(lex("fn main() {}").unwrap(), "src/main.gb").unwrap();
        assert_eq!(program.functions[0].source_name, "src/main.gb");
    }

    #[test]
    fn parses_option_result_match_and_try() {
        let source = "fn load() -> Result<string,string> { return Ok(\"yes\"); } fn main() { let x: Result<string,string> = load(); match x { Ok(v) => { print(v); } Err(e) => { print(e); } } }";
        let program = parse(lex(source).unwrap()).unwrap();
        assert_eq!(program.functions[0].return_type, Type::ResultString);
        assert!(matches!(program.functions[1].body[1].kind, StmtKind::Match { .. }));
    }

    #[test]
    fn parses_try_postfix() {
        let source = "fn load() -> Result<string,string> { return Ok(\"yes\"); } fn wrapper() -> Result<string,string> { let x: string = load()?; return Ok(x); } fn main() {}";
        let program = parse(lex(source).unwrap()).unwrap();
        match &program.functions[1].body[0].kind {
            StmtKind::Let { value: Expr::Try(_), .. } => {}
            _ => panic!("expected try expression"),
        }
    }

    #[test]
    fn reports_source_aware_syntax_errors() {
        let error = parse_named(lex("fn main( {").unwrap(), "src/main.gb").unwrap_err();
        assert_eq!(error.code, "E0100");
        assert_eq!(error.source_name.as_deref(), Some("src/main.gb"));
        assert!(error.span.is_some());
    }
}
