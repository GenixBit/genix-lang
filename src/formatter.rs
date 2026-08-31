use std::fs;
use std::path::{Path, PathBuf};

const INDENT: &str = "    ";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(String),
    Number(String),
    StringLiteral(String),
    Comment(String),
    Symbol(String),
}

impl Token {
    fn text(&self) -> &str {
        match self {
            Token::Word(value)
            | Token::Number(value)
            | Token::StringLiteral(value)
            | Token::Comment(value)
            | Token::Symbol(value) => value,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FormatSummary {
    pub files: usize,
    pub changed: Vec<PathBuf>,
}

pub fn format_target(target: &Path, check: bool) -> Result<FormatSummary, String> {
    let files = collect_targets(target)?;
    let mut changed = Vec::new();

    for path in &files {
        let source = fs::read_to_string(path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let formatted = format_source(&source)?;
        if formatted != source {
            changed.push(path.clone());
            if !check {
                fs::write(path, formatted)
                    .map_err(|error| format!("could not write {}: {error}", path.display()))?;
            }
        }
    }

    Ok(FormatSummary {
        files: files.len(),
        changed,
    })
}

pub fn format_source(source: &str) -> Result<String, String> {
    let tokens = tokenize(source)?;
    let mut printer = Printer::new();

    for (index, token) in tokens.iter().enumerate() {
        let next = tokens.get(index + 1);
        printer.write_token(token, next);
    }

    printer.finish()
}

fn collect_targets(target: &Path) -> Result<Vec<PathBuf>, String> {
    if target.is_file() {
        if !is_gb_file(target) {
            return Err("gb fmt only formats .gb source files".into());
        }
        return Ok(vec![target.to_path_buf()]);
    }

    if !target.is_dir() {
        return Err(format!("format target '{}' does not exist", target.display()));
    }

    if !target.join("genix.toml").is_file() {
        return Err(format!(
            "no genix.toml found in '{}'; pass a .gb file or a Genix project root",
            target.display()
        ));
    }

    let mut files = Vec::new();
    collect_gb_files(&target.join("src"), &mut files)?;
    collect_gb_files(&target.join("tests"), &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_gb_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)
        .map_err(|error| format!("could not read formatter directory {}: {error}", dir.display()))?
    {
        let entry = entry.map_err(|error| format!("could not read formatter directory entry: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_gb_files(&path, files)?;
        } else if is_gb_file(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn is_gb_file(path: &Path) -> bool {
    path.extension().and_then(|value| value.to_str()) == Some("gb")
}

struct Printer {
    output: String,
    line: String,
    indent: usize,
    generic_depth: usize,
    previous: Option<Token>,
}

impl Printer {
    fn new() -> Self {
        Self {
            output: String::new(),
            line: String::new(),
            indent: 0,
            generic_depth: 0,
            previous: None,
        }
    }

    fn write_token(&mut self, token: &Token, next: Option<&Token>) {
        match token {
            Token::Comment(comment) => {
                if !self.line.trim().is_empty() {
                    self.space();
                }
                self.line.push_str(comment.trim_end());
                self.flush_line();
            }
            Token::Word(word) | Token::Number(word) | Token::StringLiteral(word) => {
                self.write_value(word);
            }
            Token::Symbol(symbol) => self.write_symbol(symbol, next),
        }
        self.previous = Some(token.clone());
    }

    fn write_value(&mut self, value: &str) {
        if self.needs_space_before_value() {
            self.space();
        }
        self.line.push_str(value);
    }

    fn write_symbol(&mut self, symbol: &str, next: Option<&Token>) {
        match symbol {
            "{" => {
                if !self.line.trim().is_empty() && !self.line.ends_with(char::is_whitespace) {
                    self.space();
                }
                self.line.push('{');
                self.flush_line();
                self.indent += 1;
            }
            "}" => {
                if !self.line.trim().is_empty() {
                    self.flush_line();
                }
                self.indent = self.indent.saturating_sub(1);
                self.line.push('}');

                let next_text = next.map(Token::text);
                if matches!(next_text, Some("else") | Some(",") | Some(";")) {
                    if next_text == Some("else") {
                        self.space();
                    }
                } else {
                    self.flush_line();
                    if self.indent == 0 && next.is_some() {
                        self.blank_line();
                    }
                }
            }
            ";" => {
                self.trim_line();
                self.line.push(';');
                self.flush_line();
            }
            "," => {
                self.trim_line();
                self.line.push(',');
                if self.line.trim_start().starts_with('}') {
                    self.flush_line();
                } else if self.generic_depth == 0 {
                    self.space();
                }
            }
            ":" => {
                self.trim_line();
                self.line.push(':');
                self.space();
            }
            "." => {
                self.trim_line();
                self.line.push('.');
            }
            "(" => {
                if matches!(self.previous.as_ref(), Some(Token::Word(word)) if matches!(word.as_str(), "if" | "while" | "match" | "return")) {
                    self.space();
                }
                self.trim_line();
                self.line.push('(');
            }
            ")" => {
                self.trim_line();
                self.line.push(')');
            }
            "?" => {
                self.trim_line();
                self.line.push('?');
            }
            "<" if self.is_generic_open() => {
                self.trim_line();
                self.line.push('<');
                self.generic_depth += 1;
            }
            ">" if self.generic_depth > 0 => {
                self.trim_line();
                self.line.push('>');
                self.generic_depth -= 1;
            }
            "!" => {
                if matches!(self.previous.as_ref(), Some(Token::Word(word)) if matches!(word.as_str(), "if" | "while" | "return")) {
                    self.space();
                }
                self.trim_line();
                self.line.push('!');
            }
            "-" if self.is_unary_minus() => {
                if matches!(self.previous.as_ref(), Some(Token::Word(word)) if matches!(word.as_str(), "return" | "if" | "while")) {
                    self.space();
                }
                self.trim_line();
                self.line.push('-');
            }
            "=" | "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||" | "+" | "-" | "*" | "/" | "->" | "=>" => {
                self.trim_line();
                self.space();
                self.line.push_str(symbol);
                self.space();
            }
            _ => {
                self.trim_line();
                self.line.push_str(symbol);
            }
        }
    }

    fn is_generic_open(&self) -> bool {
        matches!(
            self.previous.as_ref(),
            Some(Token::Word(word)) if matches!(word.as_str(), "Option" | "Result")
        ) || self.generic_depth > 0
    }

    fn is_unary_minus(&self) -> bool {
        match self.previous.as_ref() {
            None => true,
            Some(Token::Symbol(symbol)) => matches!(
                symbol.as_str(),
                "(" | "{" | "," | ":" | "=" | "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||" | "+" | "-" | "*" | "/" | "->" | "=>"
            ),
            Some(Token::Word(word)) => matches!(word.as_str(), "return" | "if" | "while"),
            _ => false,
        }
    }

    fn needs_space_before_value(&self) -> bool {
        if self.line.is_empty() || self.line.ends_with(char::is_whitespace) {
            return false;
        }
        match self.previous.as_ref() {
            None => false,
            Some(Token::Symbol(symbol)) => !matches!(symbol.as_str(), "(" | "." | "<" | "!" | "-"),
            Some(Token::Comment(_)) => false,
            _ => true,
        }
    }

    fn space(&mut self) {
        if !self.line.is_empty() && !self.line.ends_with(char::is_whitespace) {
            self.line.push(' ');
        }
    }

    fn trim_line(&mut self) {
        while self.line.ends_with(char::is_whitespace) {
            self.line.pop();
        }
    }

    fn flush_line(&mut self) {
        self.trim_line();
        if self.line.is_empty() {
            return;
        }
        for _ in 0..self.indent {
            self.output.push_str(INDENT);
        }
        self.output.push_str(&self.line);
        self.output.push('\n');
        self.line.clear();
    }

    fn blank_line(&mut self) {
        if !self.output.ends_with("\n\n") && !self.output.is_empty() {
            self.output.push('\n');
        }
    }

    fn finish(mut self) -> Result<String, String> {
        if !self.line.trim().is_empty() {
            self.flush_line();
        }
        while self.output.ends_with("\n\n") {
            self.output.pop();
        }
        if !self.output.is_empty() && !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        Ok(self.output)
    }
}

fn tokenize(source: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0usize;

    while i < chars.len() {
        let ch = chars[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            let start = i;
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            tokens.push(Token::Comment(chars[start..i].iter().collect()));
            continue;
        }

        if ch == '"' {
            let start = i;
            i += 1;
            let mut escaped = false;
            while i < chars.len() {
                let current = chars[i];
                i += 1;
                if escaped {
                    escaped = false;
                    continue;
                }
                if current == '\\' {
                    escaped = true;
                    continue;
                }
                if current == '"' {
                    break;
                }
            }
            if i > chars.len() || chars.get(i.saturating_sub(1)) != Some(&'"') {
                return Err("formatter found an unterminated string literal".into());
            }
            tokens.push(Token::StringLiteral(chars[start..i].iter().collect()));
            continue;
        }

        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(Token::Word(chars[start..i].iter().collect()));
            continue;
        }

        if ch.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            tokens.push(Token::Number(chars[start..i].iter().collect()));
            continue;
        }

        if i + 1 < chars.len() {
            let pair: String = chars[i..i + 2].iter().collect();
            if matches!(pair.as_str(), "->" | "=>" | "==" | "!=" | "<=" | ">=" | "&&" | "||") {
                tokens.push(Token::Symbol(pair));
                i += 2;
                continue;
            }
        }

        tokens.push(Token::Symbol(ch.to_string()));
        i += 1;
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_functions_and_operators() {
        let source = "fn add(a:int,b:int)->int{return a+b;}fn main(){let x:int=add(1,2);print(x);}";
        let expected = "fn add(a: int, b: int) -> int {\n    return a + b;\n}\n\nfn main() {\n    let x: int = add(1, 2);\n    print(x);\n}\n";
        assert_eq!(format_source(source).unwrap(), expected);
    }

    #[test]
    fn preserves_comments_and_string_contents() {
        let source = "fn main(){// keep this\nlet x:string=\"a   b\";// trailing\nprint(x);}";
        let formatted = format_source(source).unwrap();
        assert!(formatted.contains("    // keep this\n"));
        assert!(formatted.contains("\"a   b\"; // trailing\n"));
    }

    #[test]
    fn formats_option_result_and_match() {
        let source = "fn load()->Result<string,string>{return Ok(\"x\");}fn main(){let x:Option<string>=None;match x{Some(v)=>{print(v);}None=>{print(\"none\");}}}";
        let formatted = format_source(source).unwrap();
        assert!(formatted.contains("fn load() -> Result<string,string> {"));
        assert!(formatted.contains("let x: Option<string> = None;"));
        assert!(formatted.contains("Some(v) => {"));
        assert!(formatted.contains("None => {"));
    }

    #[test]
    fn formats_test_syntax_and_is_idempotent() {
        let source = "test \"math works\"{assert(2+2==4);pass();}";
        let once = format_source(source).unwrap();
        let twice = format_source(&once).unwrap();
        assert_eq!(once, twice);
        assert_eq!(
            once,
            "test \"math works\" {\n    assert(2 + 2 == 4);\n    pass();\n}\n"
        );
    }
}
