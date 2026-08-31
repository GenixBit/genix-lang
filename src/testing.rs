use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ast::{Expr, Function, Param, Program, Stmt, Type};
use crate::diagnostics::{Diagnostic, Span};
use crate::{interpreter, lexer, parser, project, typechecker};

#[derive(Debug, Clone)]
struct RawTest {
    name: String,
    body: String,
    source_name: String,
    source: String,
    body_start: usize,
    block_span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestSiteKind {
    Assert,
    Fail,
}

#[derive(Debug, Clone)]
struct TestSite {
    id: usize,
    kind: TestSiteKind,
    span: Span,
}

#[derive(Debug, Clone)]
struct ExpandedTest {
    source: String,
    sites: Vec<TestSite>,
}

#[derive(Debug, Clone)]
struct CompiledTest {
    name: String,
    function_name: String,
    source_name: String,
    source: String,
    block_span: Span,
    sites: Vec<TestSite>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestFailureSignal {
    Assertion { site_id: usize },
    Explicit { site_id: usize, message: String },
}

pub fn run(target: &Path) -> Result<(), String> {
    let (mut program, sources) = if is_gb_file(target) {
        load_standalone(target)?
    } else {
        load_project_tests(target)?
    };

    ensure_main(&mut program);
    ensure_test_trap_signature(&mut program);
    let trap_root = unique_trap_root();

    let mut compiled = Vec::new();
    for (source_name, source) in sources {
        let (stripped, tests) = extract_tests(&source, &source_name)?;
        append_helper_functions(&mut program, &stripped, &source_name)?;

        for raw in tests {
            let function_name = format!("__genix_test_{}", compiled.len());
            let expanded = expand_test_builtins(&raw, &trap_root)?;
            let wrapper = format!("fn {function_name}() {{\n{}\n}}\n", expanded.source);
            let parsed = parse_diagnostic(&wrapper, &raw.source_name)?;
            let function = parsed
                .functions
                .into_iter()
                .next()
                .ok_or_else(|| format!("{}: test '{}' produced no executable body", raw.source_name, raw.name))?;
            program.functions.push(function);
            compiled.push(CompiledTest {
                name: raw.name,
                function_name,
                source_name: raw.source_name,
                source: raw.source,
                block_span: raw.block_span,
                sites: expanded.sites,
            });
        }
    }

    if compiled.is_empty() {
        println!("Genix Test Runner\n");
        println!("0 tests discovered");
        return Ok(());
    }

    typechecker::check(&program).map_err(|error| {
        let clean = error.strip_prefix("type error: ").unwrap_or(&error);
        format!("test type error: {clean}")
    })?;

    println!("Genix Test Runner\n");
    let mut passed = 0usize;
    let mut failed = 0usize;

    for test in &compiled {
        match execute_test(&program, test) {
            Ok(()) => {
                passed += 1;
                println!("✓ {}", test.name);
            }
            Err(error) => {
                failed += 1;
                println!("✗ {}", test.name);
                if let Some(signal) = decode_test_trap(&error, &trap_root) {
                    print!("{}", render_test_failure(test, signal));
                } else {
                    println!("  runtime error: {error}");
                    println!(
                        "  at {}:{}:{}",
                        test.source_name, test.block_span.line, test.block_span.column
                    );
                }
            }
        }
    }

    println!();
    println!("{passed} passed");
    println!("{failed} failed");

    if failed == 0 {
        Ok(())
    } else {
        Err(format!("test suite failed: {failed} test(s) failed"))
    }
}

fn load_project_tests(target: &Path) -> Result<(Program, Vec<(String, String)>), String> {
    let loaded = project::load_project(target)?;
    let tests_dir = loaded.root.join("tests");
    let mut files = Vec::new();
    collect_gb_files(&tests_dir, &mut files)?;
    files.sort();

    let mut sources = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("could not read test file {}: {error}", path.display()))?;
        sources.push((path.display().to_string(), strip_import_lines(&source)));
    }

    Ok((loaded.program, sources))
}

fn load_standalone(target: &Path) -> Result<(Program, Vec<(String, String)>), String> {
    if !target.is_file() {
        return Err(format!("test target '{}' is not a file", target.display()));
    }
    let source = fs::read_to_string(target)
        .map_err(|error| format!("could not read {}: {error}", target.display()))?;
    let source_name = target.display().to_string();
    Ok((Program { functions: Vec::new() }, vec![(source_name, source)]))
}

fn append_helper_functions(program: &mut Program, source: &str, source_name: &str) -> Result<(), String> {
    let source = strip_import_lines(source);
    if !contains_genix_code(&source) {
        return Ok(());
    }
    let parsed = parse_diagnostic(&source, source_name)?;
    for function in parsed.functions {
        if function.name == "main" {
            return Err(format!("{source_name}: test helper files cannot define fn main()"));
        }
        program.functions.push(function);
    }
    Ok(())
}

fn parse_diagnostic(source: &str, source_name: &str) -> Result<Program, String> {
    let tokens = lexer::lex_diagnostic(source).map_err(|diagnostic| {
        diagnostic
            .with_source_name(source_name.to_string())
            .render(Some(source))
    })?;
    parser::parse_named(tokens, source_name)
        .map_err(|diagnostic| diagnostic.render(Some(source)))
}

fn ensure_main(program: &mut Program) {
    if program.functions.iter().any(|function| function.name == "main") {
        return;
    }
    program.functions.push(Function {
        name: "main".to_string(),
        params: Vec::new(),
        return_type: Type::Void,
        body: Vec::new(),
    });
}

fn ensure_test_trap_signature(program: &mut Program) {
    if program.functions.iter().any(|function| function.name == "fs.read_text") {
        return;
    }
    program.functions.push(Function {
        name: "fs.read_text".to_string(),
        params: vec![Param {
            name: "path".to_string(),
            ty: Type::String,
        }],
        return_type: Type::String,
        body: vec![Stmt::Return(Some(Expr::String(String::new())))],
    });
}

fn execute_test(program: &Program, test: &CompiledTest) -> Result<(), String> {
    let mut isolated = program.clone();
    isolated.functions.retain(|function| function.name != "main");
    let position = isolated
        .functions
        .iter()
        .position(|function| function.name == test.function_name)
        .ok_or_else(|| format!("internal test runner error: missing {}", test.function_name))?;
    let mut function = isolated.functions.remove(position);
    function.name = "main".to_string();
    isolated.functions.push(function);
    interpreter::execute(&isolated)
}

fn render_test_failure(test: &CompiledTest, signal: TestFailureSignal) -> String {
    let (site_id, code, message, label) = match signal {
        TestFailureSignal::Assertion { site_id } => (
            site_id,
            "T0001",
            "assertion failed".to_string(),
            "assertion evaluated to false".to_string(),
        ),
        TestFailureSignal::Explicit { site_id, message } => (
            site_id,
            "T0002",
            format!("test failed: {}", message.replace('\n', "\\n")),
            "explicit test failure".to_string(),
        ),
    };

    let Some(site) = test.sites.iter().find(|site| site.id == site_id) else {
        return format!("  test failure at {} (unknown site {site_id})\n", test.source_name);
    };

    let expected_kind = if code == "T0001" { TestSiteKind::Assert } else { TestSiteKind::Fail };
    if site.kind != expected_kind {
        return format!("  test failure at {} (site kind mismatch)\n", test.source_name);
    }

    Diagnostic::new(code, message)
        .with_location(test.source_name.clone(), site.span)
        .with_label(label)
        .render(Some(&test.source))
}

fn unique_trap_root() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!(".genix-test-trap-{}-{nanos}", process::id())
}

fn decode_test_trap(error: &str, trap_root: &str) -> Option<TestFailureSignal> {
    let needle = format!("{trap_root}/");
    let start = error.find(&needle)? + needle.len();
    let end = error.rfind("') failed:").unwrap_or(error.len());
    if start >= end {
        return None;
    }
    let payload = &error[start..end];

    if let Some(id) = payload.strip_prefix("ASSERT/") {
        return Some(TestFailureSignal::Assertion {
            site_id: id.parse().ok()?,
        });
    }

    if let Some(rest) = payload.strip_prefix("FAIL/") {
        let mut parts = rest.splitn(2, '/');
        let site_id = parts.next()?.parse().ok()?;
        let message = parts.next().unwrap_or_default().to_string();
        return Some(TestFailureSignal::Explicit { site_id, message });
    }

    None
}

fn collect_gb_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)
        .map_err(|error| format!("could not read test directory {}: {error}", dir.display()))?
    {
        let entry = entry.map_err(|error| format!("could not read test directory entry: {error}"))?;
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

fn contains_genix_code(source: &str) -> bool {
    source.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && !trimmed.starts_with("//")
    })
}

fn strip_import_lines(source: &str) -> String {
    let mut out = String::new();
    for line in source.lines() {
        if line.trim().starts_with("import ") {
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn extract_tests(source: &str, source_name: &str) -> Result<(String, Vec<RawTest>), String> {
    let mut tests = Vec::new();
    let mut regions = Vec::new();
    let mut i = 0usize;
    let mut depth = 0usize;

    while i < source.len() {
        if source[i..].starts_with("//") {
            i = skip_line_comment(source, i);
            continue;
        }
        let ch = next_char(source, i);
        if ch == '"' {
            i = skip_string(source, i)?;
            continue;
        }
        if depth == 0 && starts_keyword(source, i, "test") {
            let start = i;
            i += 4;
            i = skip_ws(source, i);
            if i >= source.len() || next_char(source, i) != '"' {
                return Err(format!("{source_name}: expected quoted test name after 'test'"));
            }
            let (name, after_name) = parse_quoted(source, i)?;
            i = skip_ws(source, after_name);
            if i >= source.len() || next_char(source, i) != '{' {
                return Err(format!("{source_name}: expected '{{' after test \"{name}\""));
            }
            let open = i;
            let close = find_matching(source, open, '{', '}')?;
            let body_start = open + 1;
            let body = source[body_start..close].to_string();
            let end = close + 1;
            tests.push(RawTest {
                name,
                body,
                source_name: source_name.to_string(),
                source: source.to_string(),
                body_start,
                block_span: span_for_range(source, start, end),
            });
            regions.push((start, end));
            i = end;
            continue;
        }
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        i += ch.len_utf8();
    }

    let mut stripped = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for (start, end) in regions {
        stripped.push_str(&source[cursor..start]);
        for ch in source[start..end].chars() {
            if ch == '\n' {
                stripped.push('\n');
            } else {
                stripped.push(' ');
            }
        }
        cursor = end;
    }
    stripped.push_str(&source[cursor..]);
    Ok((stripped, tests))
}

fn expand_test_builtins(raw: &RawTest, trap_root: &str) -> Result<ExpandedTest, String> {
    let source = &raw.body;
    let mut out = String::with_capacity(source.len());
    let mut sites = Vec::new();
    let mut i = 0usize;
    let mut site_index = 0usize;

    while i < source.len() {
        if source[i..].starts_with("//") {
            let end = skip_line_comment(source, i);
            out.push_str(&source[i..end]);
            i = end;
            continue;
        }
        let ch = next_char(source, i);
        if ch == '"' {
            let end = skip_string(source, i)?;
            out.push_str(&source[i..end]);
            i = end;
            continue;
        }

        let builtin = ["assert", "fail", "pass"]
            .into_iter()
            .find(|name| starts_keyword(source, i, name));

        if let Some(name) = builtin {
            let after_name = skip_ws(source, i + name.len());
            if after_name < source.len() && next_char(source, after_name) == '(' {
                let close = find_matching(source, after_name, '(', ')')?;
                let raw_args = &source[after_name + 1..close];
                let args = raw_args.trim();
                let leading = raw_args.len().saturating_sub(raw_args.trim_start().len());
                let trailing = raw_args.len().saturating_sub(raw_args.trim_end().len());
                let mut end = skip_ws(source, close + 1);
                if end < source.len() && next_char(source, end) == ';' {
                    end += 1;
                }

                match name {
                    "assert" => {
                        if args.is_empty() {
                            return Err("assert(...) requires a boolean condition".into());
                        }
                        let arg_start = after_name + 1 + leading;
                        let arg_end = close.saturating_sub(trailing);
                        sites.push(TestSite {
                            id: site_index,
                            kind: TestSiteKind::Assert,
                            span: span_for_range(
                                &raw.source,
                                raw.body_start + arg_start,
                                raw.body_start + arg_end,
                            ),
                        });
                        out.push_str(&format!(
                            "if !({args}) {{ let __genix_assert_trap_{site_index}: string = fs.read_text(\"{trap_root}/ASSERT/{site_index}\"); }}"
                        ));
                    }
                    "fail" => {
                        if args.is_empty() {
                            return Err("fail(...) requires a string message".into());
                        }
                        sites.push(TestSite {
                            id: site_index,
                            kind: TestSiteKind::Fail,
                            span: span_for_range(
                                &raw.source,
                                raw.body_start + i,
                                raw.body_start + end,
                            ),
                        });
                        out.push_str(&format!(
                            "{{ let __genix_fail_trap_{site_index}: string = fs.read_text(\"{trap_root}/FAIL/{site_index}/\" + ({args})); }}"
                        ));
                    }
                    "pass" => {
                        if !args.is_empty() {
                            return Err("pass() does not take arguments".into());
                        }
                        out.push_str("{}");
                    }
                    _ => unreachable!(),
                }
                site_index += 1;
                i = end;
                continue;
            }
        }

        out.push(ch);
        i += ch.len_utf8();
    }

    Ok(ExpandedTest { source: out, sites })
}

fn span_for_range(source: &str, start: usize, end: usize) -> Span {
    let start = start.min(source.len());
    let end = end.max(start + usize::from(start < source.len())).min(source.len());
    let prefix = &source[..start];
    let line = prefix.chars().filter(|ch| *ch == '\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map(|(_, tail)| tail.chars().count() + 1)
        .unwrap_or_else(|| prefix.chars().count() + 1);

    let selected = &source[start..end];
    let newline_count = selected.chars().filter(|ch| *ch == '\n').count();
    if newline_count == 0 {
        return Span::single(line, column, selected.chars().count().max(1));
    }

    let end_line = line + newline_count;
    let end_column = selected
        .rsplit_once('\n')
        .map(|(_, tail)| tail.chars().count().max(1))
        .unwrap_or(1);
    Span::between(line, column, end_line, end_column)
}

fn find_matching(source: &str, open: usize, open_ch: char, close_ch: char) -> Result<usize, String> {
    let mut depth = 0usize;
    let mut i = open;
    while i < source.len() {
        if source[i..].starts_with("//") {
            i = skip_line_comment(source, i);
            continue;
        }
        let ch = next_char(source, i);
        if ch == '"' {
            i = skip_string(source, i)?;
            continue;
        }
        if ch == open_ch {
            depth += 1;
        } else if ch == close_ch {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Ok(i);
            }
        }
        i += ch.len_utf8();
    }
    Err(format!("unterminated '{open_ch}' block in test source"))
}

fn parse_quoted(source: &str, start: usize) -> Result<(String, usize), String> {
    let mut value = String::new();
    let mut i = start + 1;
    while i < source.len() {
        let ch = next_char(source, i);
        if ch == '"' {
            return Ok((value, i + 1));
        }
        if ch == '\\' {
            let next_index = i + 1;
            if next_index >= source.len() {
                return Err("unterminated escape in test name".into());
            }
            let escaped = next_char(source, next_index);
            value.push(match escaped {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '"' => '"',
                '\\' => '\\',
                other => other,
            });
            i = next_index + escaped.len_utf8();
            continue;
        }
        value.push(ch);
        i += ch.len_utf8();
    }
    Err("unterminated test name string".into())
}

fn skip_string(source: &str, start: usize) -> Result<usize, String> {
    let mut i = start + 1;
    while i < source.len() {
        let ch = next_char(source, i);
        if ch == '\\' {
            i += 1;
            if i < source.len() {
                let escaped = next_char(source, i);
                i += escaped.len_utf8();
            }
            continue;
        }
        i += ch.len_utf8();
        if ch == '"' {
            return Ok(i);
        }
    }
    Err("unterminated string in test source".into())
}

fn skip_line_comment(source: &str, start: usize) -> usize {
    match source[start..].find('\n') {
        Some(offset) => start + offset,
        None => source.len(),
    }
}

fn skip_ws(source: &str, mut i: usize) -> usize {
    while i < source.len() {
        let ch = next_char(source, i);
        if !ch.is_whitespace() {
            break;
        }
        i += ch.len_utf8();
    }
    i
}

fn starts_keyword(source: &str, i: usize, keyword: &str) -> bool {
    if !source[i..].starts_with(keyword) {
        return false;
    }
    if i > 0 {
        if let Some(previous) = source[..i].chars().next_back() {
            if is_identifier_char(previous) {
                return false;
            }
        }
    }
    let end = i + keyword.len();
    if end < source.len() {
        if let Some(next) = source[end..].chars().next() {
            if is_identifier_char(next) {
                return false;
            }
        }
    }
    true
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn next_char(source: &str, i: usize) -> char {
    source[i..].chars().next().expect("index must be inside source")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_test(body: &str) -> RawTest {
        RawTest {
            name: "demo".to_string(),
            body: body.to_string(),
            source_name: "tests/demo.gb".to_string(),
            source: body.to_string(),
            body_start: 0,
            block_span: span_for_range(body, 0, body.len()),
        }
    }

    #[test]
    fn extracts_nested_test_blocks_without_touching_helpers() {
        let source = "fn helper() -> int { return 2; }\n\ntest \"math works\" {\n    if true { assert(helper() == 2); }\n}\n";
        let (stripped, tests) = extract_tests(source, "tests/math.gb").unwrap();
        assert!(stripped.contains("fn helper"));
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].name, "math works");
        assert_eq!(tests[0].block_span.line, 3);
    }

    #[test]
    fn expands_test_builtins_without_division_sentinel() {
        let raw = raw_test("assert(2 + 2 == 4); fail(\"boom\"); pass();");
        let expanded = expand_test_builtins(&raw, ".genix-test-trap-unit").unwrap();
        assert!(expanded.source.contains("fs.read_text"));
        assert!(expanded.source.contains("ASSERT/0"));
        assert!(expanded.source.contains("FAIL/1/"));
        assert!(!expanded.source.contains("1 / 0"));
        assert_eq!(expanded.sites.len(), 2);
    }

    #[test]
    fn decodes_assert_and_fail_but_not_runtime_errors() {
        let root = ".genix-test-trap-unit";
        let assert_error = format!(
            "fs.read_text('{root}/ASSERT/4') failed: no such file"
        );
        assert_eq!(
            decode_test_trap(&assert_error, root),
            Some(TestFailureSignal::Assertion { site_id: 4 })
        );

        let fail_error = format!(
            "fs.read_text('{root}/FAIL/7/expected true') failed: no such file"
        );
        assert_eq!(
            decode_test_trap(&fail_error, root),
            Some(TestFailureSignal::Explicit {
                site_id: 7,
                message: "expected true".to_string(),
            })
        );
        assert_eq!(decode_test_trap("division by zero", root), None);
    }
}
