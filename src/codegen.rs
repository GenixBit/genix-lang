use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ast::{BinaryOp, Type, UnaryOp};
use crate::ir::{Expr, ExprKind, Function, Program, Stmt};

const REQUIRED_RUNTIME_ABI: &str = "1";

#[derive(Debug, Clone)]
pub struct NativeArtifact {
    pub source: PathBuf,
    pub executable: PathBuf,
    pub compiler: String,
    pub release: bool,
    pub runtime_root: PathBuf,
}

pub fn build_native(
    program: &Program,
    project_root: &Path,
    project_name: &str,
    release: bool,
) -> Result<NativeArtifact, String> {
    let build_dir = project_root.join("build");
    fs::create_dir_all(&build_dir)
        .map_err(|error| format!("could not create build directory: {error}"))?;

    let source = build_dir.join(format!("{project_name}.c"));
    let executable_name = if cfg!(windows) {
        format!("{project_name}.exe")
    } else {
        project_name.to_string()
    };
    let executable = build_dir.join(executable_name);

    let runtime_root = resolve_runtime_root(project_root)?;
    let runtime_include = runtime_root.join("include");
    let runtime_source = runtime_root.join("src/runtime.c");

    let generated = emit_c(program)?;
    fs::write(&source, generated)
        .map_err(|error| format!("could not write native C source {}: {error}", source.display()))?;

    let compiler = compile_c(
        &source,
        &runtime_source,
        &runtime_include,
        &executable,
        release,
    )?;

    Ok(NativeArtifact {
        source,
        executable,
        compiler,
        release,
        runtime_root,
    })
}

pub fn emit_c(program: &Program) -> Result<String, String> {
    Generator::new().generate(program)
}

struct Generator {
    output: String,
    indent: usize,
}

impl Generator {
    fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    fn generate(mut self, program: &Program) -> Result<String, String> {
        self.output
            .push_str("/* Generated from Genix IR by the native C11 backend. */\n");
        self.output.push_str("#include <genix/runtime.h>\n");
        self.output.push_str("#include <stdbool.h>\n");
        self.output.push_str("#include <stdint.h>\n\n");

        for function in &program.functions {
            self.output.push_str(&self.function_signature(function, true));
            self.output.push_str(";\n");
        }
        self.output.push('\n');

        for function in &program.functions {
            self.emit_function(function)?;
            self.output.push('\n');
        }

        self.output.push_str("int main(void) {\n");
        self.output.push_str("    gb_runtime_init();\n");
        self.output.push_str("    gb_fn_main();\n");
        self.output.push_str("    gb_runtime_shutdown();\n");
        self.output.push_str("    return 0;\n");
        self.output.push_str("}\n");

        Ok(self.output)
    }

    fn function_signature(&self, function: &Function, is_static: bool) -> String {
        let prefix = if is_static { "static " } else { "" };
        let params = if function.params.is_empty() {
            "void".to_string()
        } else {
            function
                .params
                .iter()
                .map(|param| format!("{} {}", c_type(param.ty), c_variable_name(&param.name)))
                .collect::<Vec<_>>()
                .join(", ")
        };

        format!(
            "{prefix}{} {}({params})",
            c_type(function.return_type),
            c_function_name(&function.name)
        )
    }

    fn emit_function(&mut self, function: &Function) -> Result<(), String> {
        self.output.push_str(&self.function_signature(function, true));
        self.output.push_str(" {\n");
        self.indent = 1;
        for statement in &function.body {
            self.emit_statement(statement)?;
        }
        self.indent = 0;
        self.output.push_str("}\n");
        Ok(())
    }

    fn emit_statement(&mut self, statement: &Stmt) -> Result<(), String> {
        match statement {
            Stmt::Let { name, value, ty, .. } => {
                let code = self.emit_expr(value)?;
                self.line(format!("{} {} = {};", c_type(*ty), c_variable_name(name), code));
            }
            Stmt::Assign { name, value } => {
                let code = self.emit_expr(value)?;
                self.line(format!("{} = {};", c_variable_name(name), code));
            }
            Stmt::Print(expr) => {
                let code = self.emit_expr(expr)?;
                let statement = match expr.ty {
                    Type::Int => format!("gb_print_int({code});"),
                    Type::Float => format!("gb_print_float({code});"),
                    Type::Bool => format!("gb_print_bool({code});"),
                    Type::String => format!("gb_print_string({code});"),
                    Type::Void => return Err("native backend: cannot print a void expression".into()),
                };
                self.line(statement);
            }
            Stmt::Expr(expr) => {
                let code = self.emit_expr(expr)?;
                self.line(format!("{code};"));
            }
            Stmt::Return(Some(expr)) => {
                let code = self.emit_expr(expr)?;
                self.line(format!("return {code};"));
            }
            Stmt::Return(None) => self.line("return;"),
            Stmt::If { condition, then_branch, else_branch } => {
                let condition = self.emit_expr(condition)?;
                self.line(format!("if ({condition}) {{"));
                self.indent += 1;
                for statement in then_branch {
                    self.emit_statement(statement)?;
                }
                self.indent -= 1;
                if let Some(else_branch) = else_branch {
                    self.line("} else {");
                    self.indent += 1;
                    for statement in else_branch {
                        self.emit_statement(statement)?;
                    }
                    self.indent -= 1;
                }
                self.line("}");
            }
            Stmt::While { condition, body } => {
                let condition = self.emit_expr(condition)?;
                self.line(format!("while ({condition}) {{"));
                self.indent += 1;
                for statement in body {
                    self.emit_statement(statement)?;
                }
                self.indent -= 1;
                self.line("}");
            }
            Stmt::Block(body) => {
                self.line("{");
                self.indent += 1;
                for statement in body {
                    self.emit_statement(statement)?;
                }
                self.indent -= 1;
                self.line("}");
            }
        }
        Ok(())
    }

    fn emit_expr(&self, expr: &Expr) -> Result<String, String> {
        match &expr.kind {
            ExprKind::Integer(value) => Ok(format!("INT64_C({value})")),
            ExprKind::Float(value) => Ok(format_float(*value)),
            ExprKind::Bool(value) => Ok(if *value { "true" } else { "false" }.into()),
            ExprKind::String(value) => Ok(format!("\"{}\"", escape_c_string(value))),
            ExprKind::Variable(name) => Ok(c_variable_name(name)),
            ExprKind::Call { callee, arguments } => {
                let args = arguments
                    .iter()
                    .map(|argument| self.emit_expr(argument))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");
                if let Some(symbol) = intrinsic_runtime_symbol(callee) {
                    Ok(format!("{symbol}({args})"))
                } else {
                    Ok(format!("{}({args})", c_function_name(callee)))
                }
            }
            ExprKind::Cast { expr, to } => {
                let code = self.emit_expr(expr)?;
                Ok(format!("(({})({code}))", c_type(*to)))
            }
            ExprKind::Unary { op, expr } => {
                let code = self.emit_expr(expr)?;
                match op {
                    UnaryOp::Negate => Ok(format!("(-({code}))")),
                    UnaryOp::Not => Ok(format!("(!({code}))")),
                }
            }
            ExprKind::Binary { left, op, right } => {
                let left_code = self.emit_expr(left)?;
                let right_code = self.emit_expr(right)?;
                match op {
                    BinaryOp::Add if expr.ty == Type::String => {
                        Ok(format!("gb_string_concat(({left_code}), ({right_code}))"))
                    }
                    BinaryOp::Add => Ok(format!("(({left_code}) + ({right_code}))")),
                    BinaryOp::Subtract => Ok(format!("(({left_code}) - ({right_code}))")),
                    BinaryOp::Multiply => Ok(format!("(({left_code}) * ({right_code}))")),
                    BinaryOp::Divide => Ok(format!("(({left_code}) / ({right_code}))")),
                    BinaryOp::Equal if left.ty == Type::String && right.ty == Type::String => {
                        Ok(format!("gb_string_equal(({left_code}), ({right_code}))"))
                    }
                    BinaryOp::NotEqual if left.ty == Type::String && right.ty == Type::String => {
                        Ok(format!("(!gb_string_equal(({left_code}), ({right_code})))"))
                    }
                    BinaryOp::Equal => Ok(format!("(({left_code}) == ({right_code}))")),
                    BinaryOp::NotEqual => Ok(format!("(({left_code}) != ({right_code}))")),
                    BinaryOp::Less => Ok(format!("(({left_code}) < ({right_code}))")),
                    BinaryOp::LessEqual => Ok(format!("(({left_code}) <= ({right_code}))")),
                    BinaryOp::Greater => Ok(format!("(({left_code}) > ({right_code}))")),
                    BinaryOp::GreaterEqual => Ok(format!("(({left_code}) >= ({right_code}))")),
                    BinaryOp::And => Ok(format!("(({left_code}) && ({right_code}))")),
                    BinaryOp::Or => Ok(format!("(({left_code}) || ({right_code}))")),
                }
            }
        }
    }

    fn line(&mut self, line: impl AsRef<str>) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        self.output.push_str(line.as_ref());
        self.output.push('\n');
    }
}

fn intrinsic_runtime_symbol(callee: &str) -> Option<&'static str> {
    match callee {
        "io.input" => Some("gb_input"),
        "process.env" => Some("gb_env_get"),
        "process.exit" => Some("gb_process_exit"),
        "fs.read_text" => Some("gb_fs_read_text"),
        "fs.write_text" => Some("gb_fs_write_text"),
        _ => None,
    }
}

fn resolve_runtime_root(project_root: &Path) -> Result<PathBuf, String> {
    if let Ok(value) = env::var("GENIX_RUNTIME") {
        let path = PathBuf::from(value.trim());
        if value.trim().is_empty() {
            return Err("GENIX_RUNTIME is set but empty".into());
        }
        validate_runtime_root(&path)?;
        return Ok(path);
    }

    let mut candidates = Vec::new();
    if let Some(parent) = project_root.parent() {
        candidates.push(parent.join("genix-runtime"));
    }
    if let Ok(current) = env::current_dir() {
        candidates.push(current.join("genix-runtime"));
        if let Some(parent) = current.parent() {
            candidates.push(parent.join("genix-runtime"));
        }
    }

    for candidate in candidates {
        if validate_runtime_root(&candidate).is_ok() {
            return Ok(candidate);
        }
    }

    Err("Genix runtime not found. Clone/install GenixBit/genix-runtime and set GENIX_RUNTIME to its directory".into())
}

fn validate_runtime_root(root: &Path) -> Result<(), String> {
    let header = root.join("include/genix/runtime.h");
    let source = root.join("src/runtime.c");
    if !header.is_file() || !source.is_file() {
        return Err(format!(
            "invalid Genix runtime at '{}': expected include/genix/runtime.h and src/runtime.c",
            root.display()
        ));
    }

    let header_text = fs::read_to_string(&header)
        .map_err(|error| format!("could not read runtime header {}: {error}", header.display()))?;
    let expected = format!("#define GENIX_RUNTIME_ABI_VERSION {REQUIRED_RUNTIME_ABI}");
    if !header_text.lines().any(|line| line.trim() == expected) {
        return Err(format!(
            "Genix runtime at '{}' is not ABI {}; expected '{}' in include/genix/runtime.h",
            root.display(), REQUIRED_RUNTIME_ABI, expected
        ));
    }
    Ok(())
}

fn compile_c(
    source: &Path,
    runtime_source: &Path,
    runtime_include: &Path,
    executable: &Path,
    release: bool,
) -> Result<String, String> {
    let candidates = compiler_candidates();
    let mut missing = Vec::new();

    for compiler in candidates {
        let mut command = Command::new(&compiler);
        command.arg("-std=c11");
        if release {
            command.arg("-O2");
        } else {
            command.arg("-O0").arg("-g");
        }
        command
            .arg(format!("-I{}", runtime_include.display()))
            .arg(source)
            .arg(runtime_source)
            .arg("-o")
            .arg(executable);

        match command.output() {
            Ok(output) if output.status.success() => return Ok(compiler),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                return Err(format!("native C compiler '{compiler}' failed\n{}{}", stdout, stderr));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => missing.push(compiler),
            Err(error) => return Err(format!("could not start native C compiler '{compiler}': {error}")),
        }
    }

    Err(format!(
        "no C compiler found (tried {}); install cc, clang, or gcc, or set the CC environment variable",
        missing.join(", ")
    ))
}

fn compiler_candidates() -> Vec<String> {
    let mut result = Vec::new();
    if let Ok(cc) = env::var("CC") {
        let cc = cc.trim();
        if !cc.is_empty() {
            result.push(cc.to_string());
        }
    }
    for compiler in ["cc", "clang", "gcc"] {
        if !result.iter().any(|candidate| candidate == compiler) {
            result.push(compiler.to_string());
        }
    }
    result
}

fn c_type(ty: Type) -> &'static str {
    match ty {
        Type::Int => "int64_t",
        Type::Float => "double",
        Type::Bool => "bool",
        Type::String => "const char*",
        Type::Void => "void",
    }
}

fn c_function_name(name: &str) -> String {
    format!("gb_fn_{}", sanitize_identifier(name))
}

fn c_variable_name(name: &str) -> String {
    format!("gb_v_{}", sanitize_identifier(name))
}

fn sanitize_identifier(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' { ch } else { '_' })
        .collect()
}

fn format_float(value: f64) -> String {
    if value.is_finite() {
        let mut text = format!("{value:.17}");
        while text.contains('.') && text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.push('0');
        }
        text
    } else {
        "0.0".to_string()
    }
}

fn escape_c_string(value: &str) -> String {
    let mut output = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ch if ch.is_control() => output.push_str(&format!("\\x{:02x}", ch as u32)),
            ch => output.push(ch),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ir, lexer, parser, typechecker};

    fn compile_source(source: &str) -> Program {
        let ast = parser::parse(lexer::lex(source).unwrap()).unwrap();
        typechecker::check(&ast).unwrap();
        ir::lower(&ast).unwrap()
    }

    #[test]
    fn emits_runtime_abi_calls_for_output_and_lifecycle() {
        let program = compile_source("fn main() { let x: int = 42; print(x); print(\"Genix\"); }");
        let c = emit_c(&program).unwrap();
        assert!(c.contains("#include <genix/runtime.h>"));
        assert!(c.contains("gb_runtime_init();"));
        assert!(c.contains("gb_print_int(gb_v_x);"));
        assert!(c.contains("gb_print_string(\"Genix\");"));
        assert!(c.contains("gb_runtime_shutdown();"));
    }

    #[test]
    fn emits_runtime_string_operations() {
        let program = compile_source("fn main() { let x: string = \"Ge\" + \"nix\"; print(x == \"Genix\"); }");
        let c = emit_c(&program).unwrap();
        assert!(c.contains("gb_string_concat"));
        assert!(c.contains("gb_string_equal"));
    }

    #[test]
    fn maps_stdlib_intrinsics_to_runtime_symbols() {
        assert_eq!(intrinsic_runtime_symbol("io.input"), Some("gb_input"));
        assert_eq!(intrinsic_runtime_symbol("fs.read_text"), Some("gb_fs_read_text"));
        assert_eq!(intrinsic_runtime_symbol("fs.write_text"), Some("gb_fs_write_text"));
        assert_eq!(intrinsic_runtime_symbol("process.env"), Some("gb_env_get"));
        assert_eq!(intrinsic_runtime_symbol("process.exit"), Some("gb_process_exit"));
    }
}
