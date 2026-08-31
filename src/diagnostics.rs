use std::fmt;
use std::fs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

impl Span {
    pub fn single(line: usize, column: usize, width: usize) -> Self {
        let width = width.max(1);
        Self {
            line,
            column,
            end_line: line,
            end_column: column + width - 1,
        }
    }

    pub fn between(
        line: usize,
        column: usize,
        end_line: usize,
        end_column: usize,
    ) -> Self {
        Self {
            line,
            column,
            end_line,
            end_column: end_column.max(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedLocation {
    pub source_name: String,
    pub span: Span,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub label: Option<String>,
    pub help: Option<String>,
    pub source_name: Option<String>,
    pub span: Option<Span>,
    pub related: Vec<RelatedLocation>,
}

impl Diagnostic {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            label: None,
            help: None,
            source_name: None,
            span: None,
            related: Vec::new(),
        }
    }

    pub fn syntax(message: impl Into<String>) -> Self {
        Self::new("E0100", message)
    }

    pub fn type_error(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(code, message)
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_location(mut self, source_name: impl Into<String>, span: Span) -> Self {
        self.source_name = Some(source_name.into());
        if self.span.is_none() {
            self.span = Some(span);
        }
        self
    }

    pub fn with_source_name(mut self, source_name: impl Into<String>) -> Self {
        self.source_name = Some(source_name.into());
        self
    }

    pub fn with_related(
        mut self,
        source_name: impl Into<String>,
        span: Span,
        label: impl Into<String>,
    ) -> Self {
        self.related.push(RelatedLocation {
            source_name: source_name.into(),
            span,
            label: label.into(),
        });
        self
    }

    pub fn render(&self, source: Option<&str>) -> String {
        let mut out = format!("error[{}]: {}\n", self.code, self.message);

        if let (Some(name), Some(span)) = (&self.source_name, self.span) {
            out.push_str(&format!(" --> {}:{}:{}\n", name, span.line, span.column));
            if let Some(source) = source {
                render_excerpt(&mut out, source, span, self.label.as_deref(), '^');
            }
        }

        for related in &self.related {
            out.push_str(&format!(
                " ::: {}:{}:{}\n",
                related.source_name, related.span.line, related.span.column
            ));
            if let Ok(source) = fs::read_to_string(&related.source_name) {
                render_excerpt(&mut out, &source, related.span, Some(&related.label), '-');
            }
        }

        if let Some(help) = &self.help {
            out.push_str(&format!("  = help: {help}\n"));
        }
        out
    }

    pub fn render_from_disk(&self) -> String {
        let source = self
            .source_name
            .as_deref()
            .filter(|name| *name != "<memory>")
            .and_then(|name| fs::read_to_string(name).ok());
        self.render(source.as_deref())
    }
}

fn render_excerpt(
    out: &mut String,
    source: &str,
    span: Span,
    label: Option<&str>,
    marker: char,
) {
    if let Some(line_text) = source.lines().nth(span.line.saturating_sub(1)) {
        let line_no = span.line.to_string();
        let gutter = " ".repeat(line_no.len());
        let start = span.column.saturating_sub(1);
        let width = if span.end_line == span.line {
            span.end_column.saturating_sub(span.column) + 1
        } else {
            line_text.chars().count().saturating_sub(start).max(1)
        };
        out.push_str(&format!(" {gutter} |\n"));
        out.push_str(&format!(" {line_no} | {line_text}\n"));
        out.push_str(&format!(
            " {gutter} | {}{}",
            " ".repeat(start),
            marker.to_string().repeat(width.max(1))
        ));
        if let Some(label) = label {
            out.push(' ');
            out.push_str(label);
        }
        out.push('\n');
    }
}

pub fn split_type_error(message: &str) -> (Option<String>, String) {
    let raw = message.strip_prefix("type error: ").unwrap_or(message);
    let prefix = "type error in function '";
    if let Some(rest) = message.strip_prefix(prefix) {
        if let Some(end) = rest.find("': ") {
            let function = rest[..end].to_string();
            let clean = rest[end + 3..].to_string();
            return (Some(function), clean);
        }
    }
    (None, raw.to_string())
}

pub fn type_diagnostic(message: &str, source_name: &str, source: &str) -> Diagnostic {
    let (_, clean) = split_type_error(message);
    let code = classify_type_error(&clean);
    let mut diagnostic = Diagnostic::type_error(code, clean.clone())
        .with_source_name(source_name.to_string());

    match code {
        "E0201" => {
            diagnostic = diagnostic
                .with_label("type mismatch")
                .with_help("change the expression or annotation so the types are compatible");
        }
        "E0202" => {
            diagnostic = diagnostic
                .with_label("name is not defined here")
                .with_help("check the spelling, declaration, or imported module");
        }
        "E0203" => {
            diagnostic = diagnostic
                .with_label("immutable binding")
                .with_help("declare the variable with `mut` if reassignment is intended");
        }
        "E0204" => {
            diagnostic = diagnostic
                .with_label("invalid return")
                .with_help("make every return value match the function return type");
        }
        "E0205" => {
            diagnostic = diagnostic
                .with_label("invalid or non-exhaustive match")
                .with_help("Option needs Some/None; Result needs Ok/Err");
        }
        "E0206" => {
            diagnostic = diagnostic
                .with_label("invalid '?' propagation")
                .with_help("use '?' only with Result<T,string> in a Result-returning function");
        }
        "E0207" => {
            diagnostic = diagnostic
                .with_label("function call does not match its signature")
                .with_help("check the number and types of the arguments");
        }
        "E0209" => {
            diagnostic = diagnostic
                .with_label("duplicate declaration")
                .with_help("rename or remove the duplicate declaration");
        }
        "E0210" => {
            diagnostic = diagnostic.with_help("define `fn main() { ... }` with no parameters or return type");
        }
        _ => {}
    }

    if let Some(span) = locate_type_error(&clean, source) {
        diagnostic.span = Some(span);
    }
    diagnostic
}

fn classify_type_error(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("undefined variable") || lower.contains("undefined function") {
        "E0202"
    } else if lower.contains("immutable variable") || lower.contains("cannot assign to immutable") {
        "E0203"
    } else if lower.contains("return") || lower.contains("guaranteed return") {
        "E0204"
    } else if lower.contains("match") || lower.contains("pattern") {
        "E0205"
    } else if lower.contains("'?'") || lower.contains("?'") || lower.contains("operator '?'") {
        "E0206"
    } else if lower.contains("argument") || lower.contains("expects") && lower.contains("found") {
        "E0207"
    } else if lower.contains("defined more than once")
        || lower.contains("already declared")
        || lower.contains("duplicate parameter")
    {
        "E0209"
    } else if lower.contains("fn main") || lower.contains("must define fn main") {
        "E0210"
    } else if lower.contains("expected") && lower.contains("found")
        || lower.contains("requires numeric")
        || lower.contains("requires bool")
        || lower.contains("requires a number")
        || lower.contains("comparison requires")
        || lower.contains("logical operator requires")
    {
        "E0201"
    } else if lower.contains("void")
        || lower.contains("none requires")
        || lower.contains("err(...)")
        || lower.contains("some(")
        || lower.contains("print() cannot")
    {
        "E0208"
    } else {
        "E0299"
    }
}

fn locate_type_error(message: &str, source: &str) -> Option<Span> {
    let lines: Vec<&str> = source.lines().collect();

    if let Some(name) = quoted_after(message, "initializer for '") {
        if let Some((index, line)) = lines.iter().enumerate().find(|(_, line)| {
            line.contains(&format!("let {name}")) || line.contains(&format!("mut {name}"))
        }) {
            return initializer_span(index + 1, line);
        }
    }

    if let Some(name) = quoted_after(message, "assignment to '")
        .or_else(|| quoted_after(message, "undefined variable '"))
        .or_else(|| quoted_after(message, "immutable variable '"))
        .or_else(|| quoted_after(message, "variable '"))
    {
        if let Some((index, line)) = lines.iter().enumerate().find(|(_, line)| line.contains(&name)) {
            let column = line.find(&name).unwrap_or(0) + 1;
            return Some(Span::single(index + 1, column, name.chars().count()));
        }
    }

    if let Some(name) = quoted_after(message, "function '") {
        let local = name.rsplit('.').next().unwrap_or(&name);
        if let Some((index, line)) = lines.iter().enumerate().find(|(_, line)| {
            line.contains(&format!("{name}(")) || line.contains(&format!("{local}("))
        }) {
            let needle = if line.contains(&name) { &name } else { local };
            let column = line.find(needle).unwrap_or(0) + 1;
            return Some(Span::single(index + 1, column, needle.chars().count()));
        }
    }

    let preferred = if message.contains("if condition") {
        Some("if ")
    } else if message.contains("while condition") {
        Some("while ")
    } else if message.contains("match") || message.contains("pattern") {
        Some("match ")
    } else if message.contains("return") {
        Some("return")
    } else {
        None
    };

    if let Some(needle) = preferred {
        if let Some((index, line)) = lines.iter().enumerate().find(|(_, line)| line.contains(needle)) {
            let column = line.find(needle).unwrap_or(0) + 1;
            return Some(Span::single(index + 1, column, needle.trim().chars().count().max(1)));
        }
    }

    lines
        .iter()
        .enumerate()
        .find(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            let column = line.chars().position(|ch| !ch.is_whitespace()).unwrap_or(0) + 1;
            Span::single(index + 1, column, line.trim().chars().count().max(1))
        })
}

fn initializer_span(line_number: usize, line: &str) -> Option<Span> {
    let equal = line.find('=')?;
    let after_equal = &line[equal + 1..];
    let leading = after_equal.chars().take_while(|ch| ch.is_whitespace()).count();
    let value_start = equal + 1 + leading;
    let value_text = after_equal.trim_start().trim_end_matches(';').trim_end();
    let width = value_text.chars().count().max(1);
    Some(Span::single(line_number, value_start + 1, width))
}

fn quoted_after(message: &str, prefix: &str) -> Option<String> {
    let start = message.find(prefix)? + prefix.len();
    let rest = &message[start..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error[{}]: {}", self.code, self.message)
    }
}

impl std::error::Error for Diagnostic {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_source_excerpt() {
        let diagnostic = Diagnostic::type_error("E0201", "type mismatch")
            .with_location("src/main.gb", Span::single(2, 20, 8))
            .with_label("found string")
            .with_help("use an int value");
        let rendered = diagnostic.render(Some("fn main() {\n    let age: int = \"twenty\";\n}\n"));
        assert!(rendered.contains("error[E0201]"));
        assert!(rendered.contains("src/main.gb:2:20"));
        assert!(rendered.contains("^^^^^^^^"));
        assert!(rendered.contains("help:"));
    }

    #[test]
    fn replaces_memory_source_name_without_losing_span() {
        let diagnostic = Diagnostic::new("E0001", "bad token")
            .with_location("<memory>", Span::single(1, 2, 1))
            .with_source_name("src/main.gb");
        assert_eq!(diagnostic.source_name.as_deref(), Some("src/main.gb"));
        assert_eq!(diagnostic.span.unwrap().column, 2);
    }

    #[test]
    fn maps_type_mismatch_to_initializer_value() {
        let source = "fn main() {\n    let age: int = \"twenty\";\n}\n";
        let diagnostic = type_diagnostic(
            "type error in function 'main': initializer for 'age' expected int, found string",
            "src/main.gb",
            source,
        );
        assert_eq!(diagnostic.code, "E0201");
        assert_eq!(diagnostic.message, "initializer for 'age' expected int, found string");
        assert_eq!(diagnostic.span.unwrap().line, 2);
        assert!(diagnostic.span.unwrap().column > 10);
    }

    #[test]
    fn splits_function_context_from_type_error() {
        let (function, message) = split_type_error(
            "type error in function 'math.add': initializer for 'x' expected int, found string",
        );
        assert_eq!(function.as_deref(), Some("math.add"));
        assert!(message.starts_with("initializer for 'x'"));
    }
}
