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
        let rendered = diagnostic.render(Some(
            "fn main() {\n    let age: int = \"twenty\";\n}\n",
        ));
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
    fn renders_related_locations() {
        let diagnostic = Diagnostic::type_error("E0207", "bad call")
            .with_location("src/main.gb", Span::single(2, 5, 3))
            .with_related("src/math.gb", Span::single(1, 1, 6), "function defined here");
        let rendered = diagnostic.render(Some("fn main() {\n    add();\n}\n"));
        assert!(rendered.contains("error[E0207]"));
        assert!(rendered.contains("::: src/math.gb:1:1"));
    }
}
