#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Fn,
    Let,
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
    Equal,
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
}

pub fn lex(source: &str) -> Result<Vec<Token>, String> {
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
            '=' => push_simple(&mut tokens, TokenKind::Equal, line, column, &mut i, &mut column),
            '+' => push_simple(&mut tokens, TokenKind::Plus, line, column, &mut i, &mut column),
            '-' => push_simple(&mut tokens, TokenKind::Minus, line, column, &mut i, &mut column),
            '*' => push_simple(&mut tokens, TokenKind::Star, line, column, &mut i, &mut column),
            '/' => push_simple(&mut tokens, TokenKind::Slash, line, column, &mut i, &mut column),
            '"' => {
                let start_line = line;
                let start_column = column;
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
                            return Err(format!(
                                "unterminated string at {start_line}:{start_column}"
                            ));
                        }
                        c => {
                            value.push(c);
                            i += 1;
                            column += 1;
                        }
                    }
                }

                if i >= chars.len() {
                    return Err(format!(
                        "unterminated string at {start_line}:{start_column}"
                    ));
                }

                i += 1;
                column += 1;
                tokens.push(Token {
                    kind: TokenKind::String(value),
                    line: start_line,
                    column: start_column,
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
                    } else if chars[i] == '.' && !has_dot {
                        has_dot = true;
                        i += 1;
                        column += 1;
                    } else {
                        break;
                    }
                }

                let text: String = chars[start..i].iter().collect();
                let kind = if has_dot {
                    TokenKind::Float(text.parse().map_err(|_| {
                        format!("invalid float '{text}' at {line}:{start_column}")
                    })?)
                } else {
                    TokenKind::Integer(text.parse().map_err(|_| {
                        format!("invalid integer '{text}' at {line}:{start_column}")
                    })?)
                };

                tokens.push(Token {
                    kind,
                    line,
                    column: start_column,
                });
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
                    "true" => TokenKind::True,
                    "false" => TokenKind::False,
                    _ => TokenKind::Identifier(text),
                };

                tokens.push(Token {
                    kind,
                    line,
                    column: start_column,
                });
            }
            other => {
                return Err(format!("unexpected character '{other}' at {line}:{column}"));
            }
        }
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        line,
        column,
    });

    Ok(tokens)
}

fn push_simple(
    tokens: &mut Vec<Token>,
    kind: TokenKind,
    line: usize,
    column: usize,
    i: &mut usize,
    current_column: &mut usize,
) {
    tokens.push(Token { kind, line, column });
    *i += 1;
    *current_column += 1;
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
}
