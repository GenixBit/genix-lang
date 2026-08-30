use crate::diagnostics::{Diagnostic, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Fn,
    Let,
    Mut,
    If,
    Else,
    While,
    Match,
    Return,
    True,
    False,
    Identifier(String),
    Integer(i64),
    Float(f64),
    String(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    Semicolon,
    Colon,
    Comma,
    Dot,
    Arrow,
    FatArrow,
    Question,
    Equal,
    EqualEqual,
    Bang,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,
    Plus,
    Minus,
    Star,
    Slash,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
    pub width: usize,
}

impl Token {
    pub fn span(&self) -> Span {
        Span::single(self.line, self.column, self.width)
    }
}

pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut line = 1;
    let mut column = 1;

    while i < chars.len() {
        let ch = chars[i];

        match ch {
            ' ' | '\t' | '\r' => {
                i += 1;
                column += 1;
            }
            '\n' => {
                i += 1;
                line += 1;
                column = 1;
            }
            '/' if i + 1 < chars.len() && chars[i + 1] == '/' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                    column += 1;
                }
            }
            '(' => push_simple(&mut tokens, TokenKind::LParen, line, column, &mut i, &mut column),
            ')' => push_simple(&mut tokens, TokenKind::RParen, line, column, &mut i, &mut column),
            '{' => push_simple(&mut tokens, TokenKind::LBrace, line, column, &mut i, &mut column),
            '}' => push_simple(&mut tokens, TokenKind::RBrace, line, column, &mut i, &mut column),
            ';' => push_simple(&mut tokens, TokenKind::Semicolon, line, column, &mut i, &mut column),
            ':' => push_simple(&mut tokens, TokenKind::Colon, line, column, &mut i, &mut column),
            ',' => push_simple(&mut tokens, TokenKind::Comma, line, column, &mut i, &mut column),
            '.' => push_simple(&mut tokens, TokenKind::Dot, line, column, &mut i, &mut column),
            '?' => push_simple(&mut tokens, TokenKind::Question, line, column, &mut i, &mut column),
            '-' if matches_next(&chars, i, '>') => push_double(&mut tokens, TokenKind::Arrow, line, column, &mut i, &mut column),
            '=' if matches_next(&chars, i, '>') => push_double(&mut tokens, TokenKind::FatArrow, line, column, &mut i, &mut column),
            '=' if matches_next(&chars, i, '=') => push_double(&mut tokens, TokenKind::EqualEqual, line, column, &mut i, &mut column),
            '=' => push_simple(&mut tokens, TokenKind::Equal, line, column, &mut i, &mut column),
            '!' if matches_next(&chars, i, '=') => push_double(&mut tokens, TokenKind::BangEqual, line, column, &mut i, &mut column),
            '!' => push_simple(&mut tokens, TokenKind::Bang, line, column, &mut i, &mut column),
            '<' if matches_next(&chars, i, '=') => push_double(&mut tokens, TokenKind::LessEqual, line, column, &mut i, &mut column),
            '<' => push_simple(&mut tokens, TokenKind::Less, line, column, &mut i, &mut column),
            '>' if matches_next(&chars, i, '=') => push_double(&mut tokens, TokenKind::GreaterEqual, line, column, &mut i, &mut column),
            '>' => push_simple(&mut tokens, TokenKind::Greater, line, column, &mut i, &mut column),
            '&' if matches_next(&chars, i, '&') => push_double(&mut tokens, TokenKind::AndAnd, line, column, &mut i, &mut column),
            '|' if matches_next(&chars, i, '|') => push_double(&mut tokens, TokenKind::OrOr, line, column, &mut i, &mut column),
            '+' => push_simple(&mut tokens, TokenKind::Plus, line, column, &mut i, &mut column),
            '-' => push_simple(&mut tokens, TokenKind::Minus, line, column, &mut i, &mut column),
            '*' => push_simple(&mut tokens, TokenKind::Star, line, column, &mut i, &mut column),
            '/' => push_simple(&mut tokens, TokenKind::Slash, line, column, &mut i, &mut column),
            '"' => {
                let start_line = line;
                let start_column = column;
                let start = i;
                i += 1;
                column += 1;
                let mut value = String::new();

                while i < chars.len() && chars[i] != '"' {
                    match chars[i] {
                        '\\' if i + 1 < chars.len() => {
                            let escaped = match chars[i + 1] {
                                'n' => '\n',
                                't' => '\t',
                                'r' => '\r',
                                '"' => '"',
                                '\\' => '\\',
                                other => other,
                            };
                            value.push(escaped);
                            i += 2;
                            column += 2;
                        }
                        '\n' => {
                            return Err(
                                Diagnostic::new("E0002", "unterminated string literal")
                                    .with_label("string starts here")
                                    .with_help("close the string with a double quote before the end of the line")
                                    .with_location("<memory>", Span::single(start_line, start_column, 1)),
                            )
                        }
                        c => {
                            value.push(c);
                            i += 1;
                            column += 1;
                        }
                    }
                }

                if i >= chars.len() {
                    return Err(
                        Diagnostic::new("E0002", "unterminated string literal")
                            .with_label("string starts here")
                            .with_help("add a closing double quote")
                            .with_location("<memory>", Span::single(start_line, start_column, 1)),
                    );
                }

                i += 1;
                column += 1;
                tokens.push(Token {
                    kind: TokenKind::String(value),
                    line: start_line,
                    column: start_column,
                    width: i - start,
                });
            }
            c if c.is_ascii_digit() => {
                let start = i;
                let start_column = column;
                let mut has_dot = false;

                while i < chars.len() {
                    if chars[i].is_ascii_digit() {
                        i += 1;
                        column += 1;
                    } else if chars[i] == '.' && !has_dot && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
                        has_dot = true;
                        i += 1;
                        column += 1;
                    } else {
                        break;
                    }
                }

                let text: String = chars[start..i].iter().collect();
                let width = i - start;
                let kind = if has_dot {
                    TokenKind::Float(text.parse().map_err(|_| {
                        Diagnostic::new("E0003", format!("invalid float literal '{text}'"))
                            .with_label("invalid number")
                            .with_location("<memory>", Span::single(line, start_column, width))
                    })?)
                } else {
                    TokenKind::Integer(text.parse().map_err(|_| {
                        Diagnostic::new("E0003", format!("invalid integer literal '{text}'"))
                            .with_label("invalid number")
                            .with_location("<memory>", Span::single(line, start_column, width))
                    })?)
                };

                tokens.push(Token { kind, line, column: start_column, width });
            }
            c if is_identifier_start(c) => {
                let start = i;
                let start_column = column;
                i += 1;
                column += 1;

                while i < chars.len() && is_identifier_continue(chars[i]) {
                    i += 1;
                    column += 1;
                }

                let text: String = chars[start..i].iter().collect();
                let kind = match text.as_str() {
                    "fn" => TokenKind::Fn,
                    "let" => TokenKind::Let,
                    "mut" => TokenKind::Mut,
                    "if" => TokenKind::If,
                    "else" => TokenKind::Else,
                    "while" => TokenKind::While,
                    "match" => TokenKind::Match,
                    "return" => TokenKind::Return,
                    "true" => TokenKind::True,
                    "false" => TokenKind::False,
                    _ => TokenKind::Identifier(text),
                };

                tokens.push(Token { kind, line, column: start_column, width: i - start });
            }
            '&' => {
                return Err(
                    Diagnostic::new("E0001", "unexpected '&'")
                        .with_label("single '&' is not a Genix operator")
                        .with_help("use '&&' for logical and")
                        .with_location("<memory>", Span::single(line, column, 1)),
                )
            }
            '|' => {
                return Err(
                    Diagnostic::new("E0001", "unexpected '|'")
                        .with_label("single '|' is not a Genix operator")
                        .with_help("use '||' for logical or")
                        .with_location("<memory>", Span::single(line, column, 1)),
                )
            }
            other => {
                return Err(
                    Diagnostic::new("E0001", format!("unexpected character '{other}'"))
                        .with_label("not valid Genix syntax")
                        .with_location("<memory>", Span::single(line, column, 1)),
                )
            }
        }
    }

    tokens.push(Token { kind: TokenKind::Eof, line, column, width: 1 });
    Ok(tokens)
}

fn matches_next(chars: &[char], i: usize, expected: char) -> bool {
    i + 1 < chars.len() && chars[i + 1] == expected
}

fn push_simple(tokens: &mut Vec<Token>, kind: TokenKind, line: usize, column: usize, i: &mut usize, current_column: &mut usize) {
    tokens.push(Token { kind, line, column, width: 1 });
    *i += 1;
    *current_column += 1;
}

fn push_double(tokens: &mut Vec<Token>, kind: TokenKind, line: usize, column: usize, i: &mut usize, current_column: &mut usize) {
    tokens.push(Token { kind, line, column, width: 2 });
    *i += 2;
    *current_column += 2;
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_hello_world() {
        let tokens = lex("fn main() { print(\"Hello\"); }").unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Fn));
        assert!(matches!(tokens[1].kind, TokenKind::Identifier(ref name) if name == "main"));
        assert!(tokens.iter().any(|token| matches!(token.kind, TokenKind::String(ref value) if value == "Hello")));
    }

    #[test]
    fn records_token_widths() {
        let tokens = lex("let answer = 42;").unwrap();
        assert_eq!(tokens[0].width, 3);
        assert_eq!(tokens[1].width, 6);
        assert_eq!(tokens[3].width, 2);
    }

    #[test]
    fn lexes_function_types_and_return() {
        let tokens = lex("fn add(a: int, b: int) -> int { return a + b; }").unwrap();
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Colon)));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Comma)));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Arrow)));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Return)));
    }

    #[test]
    fn lexes_match_and_try_syntax() {
        let tokens = lex("match x { Some(v) => { print(v); } None => {} } let y = load()?;").unwrap();
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Match)));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::FatArrow)));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Question)));
    }

    #[test]
    fn reports_structured_lex_errors() {
        let error = lex("fn main() { @ }").unwrap_err();
        assert_eq!(error.code, "E0001");
        assert_eq!(error.span.unwrap().line, 1);
    }
}
