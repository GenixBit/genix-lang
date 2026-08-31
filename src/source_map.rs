use std::collections::HashMap;

use crate::diagnostics::Span;

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub name: String,
    pub source: String,
}

#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    files: HashMap<String, SourceFile>,
    function_files: HashMap<String, String>,
    module_files: HashMap<String, String>,
    entry_file: Option<String>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_file(&mut self, name: impl Into<String>, source: impl Into<String>) {
        let name = name.into();
        self.files.insert(
            name.clone(),
            SourceFile {
                name,
                source: source.into(),
            },
        );
    }

    pub fn set_entry(&mut self, name: impl Into<String>) {
        self.entry_file = Some(name.into());
    }

    pub fn bind_function(&mut self, function: impl Into<String>, file: impl Into<String>) {
        self.function_files.insert(function.into(), file.into());
    }

    pub fn bind_module(&mut self, module: impl Into<String>, file: impl Into<String>) {
        self.module_files.insert(module.into(), file.into());
    }

    pub fn file(&self, name: &str) -> Option<&SourceFile> {
        self.files.get(name)
    }

    pub fn entry(&self) -> Option<&SourceFile> {
        self.entry_file
            .as_deref()
            .and_then(|name| self.files.get(name))
    }

    pub fn file_for_function(&self, function: &str) -> Option<&SourceFile> {
        self.function_files
            .get(function)
            .and_then(|name| self.files.get(name))
    }

    pub fn file_for_module(&self, module: &str) -> Option<&SourceFile> {
        self.module_files
            .get(module)
            .and_then(|name| self.files.get(name))
    }

    pub fn function_file_name(&self, function: &str) -> Option<&str> {
        self.function_files.get(function).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn locate(&self, file: &str, needle: &str) -> Option<Span> {
        let source = &self.files.get(file)?.source;
        locate_text(source, needle)
    }

    pub fn locate_function(&self, function: &str) -> Option<(String, Span)> {
        let file = self.file_for_function(function)?;
        let local_name = function.rsplit('.').next().unwrap_or(function);
        let span = locate_text(&file.source, &format!("fn {local_name}"))?;
        Some((file.name.clone(), span))
    }

    pub fn locate_module_reference(&self, module: &str) -> Option<(String, Span)> {
        let entry = self.entry()?;
        let call = format!("{module}.");
        if let Some(span) = locate_text(&entry.source, &call) {
            return Some((entry.name.clone(), Span::single(span.line, span.column, module.len())));
        }
        let import = format!("import {module}");
        locate_text(&entry.source, &import).map(|span| {
            let column = span.column + "import ".len();
            (entry.name.clone(), Span::single(span.line, column, module.len()))
        })
    }
}

pub fn locate_text(source: &str, needle: &str) -> Option<Span> {
    for (line_index, line) in source.lines().enumerate() {
        if let Some(byte_column) = line.find(needle) {
            let column = line[..byte_column].chars().count() + 1;
            return Some(Span::single(
                line_index + 1,
                column,
                needle.chars().count().max(1),
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_functions_and_module_references_to_files() {
        let mut map = SourceMap::new();
        map.add_file(
            "src/main.gb",
            "import math;\nfn main() { print(math.add(1, 2)); }\n",
        );
        map.add_file("src/math.gb", "fn add(a: int, b: int) -> int { return a + b; }\n");
        map.set_entry("src/main.gb");
        map.bind_module("math", "src/math.gb");
        map.bind_function("math.add", "src/math.gb");

        assert_eq!(map.file_for_function("math.add").unwrap().name, "src/math.gb");
        let (file, span) = map.locate_module_reference("math").unwrap();
        assert_eq!(file, "src/main.gb");
        assert_eq!(span.line, 2);
        let (definition_file, _) = map.locate_function("math.add").unwrap();
        assert_eq!(definition_file, "src/math.gb");
    }
}
