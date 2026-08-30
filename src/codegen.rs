use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ast::{BinaryOp, Type, UnaryOp};
use crate::ir::{Expr, ExprKind, Function, Program, Stmt};

#[derive(Debug, Clone)]
pub struct NativeArtifact {
    pub source: PathBuf,
    pub executable: PathBuf,
    pub compiler: String,
    pub release: bool,
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

    let generated = emit_c(program)?;
    fs::write(&source, generated)
        .map_err(|error| format!("could not write native C source {}: {error}", source.display()))?;

    let compiler = compile_c(&source, &executable, release)?;

    Ok(NativeArtifact {
        source,
        executable,
        compiler,
        release,
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
        self.output.push_str("#include <stdbool.h>\n");
        self.output.push_str("#include <stdint.h>\n");
        self.output.push_str("#include <stdio.h>\n");
        self.output.push_str("#include <stdlib.h>\n");
        self.output.push_str("#include <string.h>\n\n");

        self.emit_runtime();
        self.output.push('\n');

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
        self.output.push_str("    gb_fn_main();\n");
        self.output.push_str("    return 0;\n");
        self.output.push_str("}\n");

        Ok(self.output)
    }

    fn emit_runtime(&mut self) {
        self.output.push_str(
            "static char* gb_concat(const char* left, const char* right) {\n\
    size_t left_len = strlen(left);\n\
    size_t right_len = strlen(right);\n\
    char* result = (char*)malloc(left_len + right_len + 1);\n\
    if (result == NULL) {\n\
        fputs(\"Genix runtime error: out of memory\\n\", stderr);\n\
        exit(70);\n\
    }\n\
    memcpy(result, left, left_len);\n\
    memcpy(result + left_len, right, right_len);\n\
    result[left_len + right_len] = '\\0';\n\
    return result;\n\
}\n",
        );
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
            Stmt::Let {
                name, value, ty, ..
            } => {
                let code = self.emit_expr(value)?;
                self.line(format!(
                    "{} {} = {};",
                    c_type(*ty),
                    c_variable_name(name),
                    code
                ));
            }
            Stmt::Assign { name, value } => {
                let code = self.emit_expr(value)?;
                self.line(format!("{} = {};", c_variable_name(name), code));
            }
            Stmt::Print(expr) => {
                let code = self.emit_expr(expr)?;
                let statement = match expr.ty {
                    Type::Int => format!("printf(\"%lld\\n\", (long long)({code}));"),
                    Type::Float => format!("printf(\"%.15g\\n\", (double)({code}));"),
                    Type::Bool => {
                        format!("printf(\"%s\\n\", ({code}) ? \"true\" : \"false\");")
                    }
                    Type::String => format!("printf(\"%s\\n\", {code});"),
                    Type::Void => {
                        return Err("native backend: cannot print a void expression".into())
                    }
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
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
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
                Ok(format!("{}({args})", c_function_name(callee)))
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
                        Ok(format!("gb_concat(({left_code}), ({right_code}))"))
                    }
                    BinaryOp::Add => Ok(format!("(({left_code}) + ({right_code}))")),
                    BinaryOp::Subtract => Ok(format!("(({left_code}) - ({right_code}))")),
                    BinaryOp::Multiply => Ok(format!("(({left_code}) * ({right_code}))")),
                    BinaryOp::Divide => Ok(format!("(({left_code}) / ({right_code}))")),
                    BinaryOp::Equal | BinaryOp::NotEqual
                        if left.ty == Type::String && right.ty == Type::String =>
                    {
                        let comparison = if matches!(op, BinaryOp::Equal) { "==" } else { "!=" };
                        Ok(format!(
                            "(strcmp(({left_code}), ({right_code})) {comparison} 0)"
                        ))
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

fn compile_c(source: &Path, executable: &Path, release: bool) -> Result<String, String> {
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
        command.arg(source).arg("-o").arg(executable);

        match command.output() {
            Ok(output) if output.status.success() => return Ok(compiler),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                return Err(format!(
                    "native C compiler '{compiler}' failed\n{}{}",
                    stdout, stderr
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => missing.push(compiler),
            Err(error) => {
                return Err(format!("could not start native C compiler '{compiler}': {error}"))
            }
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
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
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
    fn emits_native_c_from_ir_for_typed_function() {
        let program = compile_source(
            "fn add(a: int, b: int) -> int { return a + b; } fn main() { let x: int = add(20, 22); print(x); }",
        );
        let c = emit_c(&program).unwrap();
        assert!(c.contains("Generated from Genix IR"));
        assert!(c.contains("static int64_t gb_fn_add"));
        assert!(c.contains("gb_fn_add(INT64_C(20), INT64_C(22))"));
        assert!(c.contains("int main(void)"));
    }

    #[test]
    fn emits_explicit_ir_numeric_casts() {
        let program = compile_source(
            "fn scale(value: float) -> float { return value; } fn main() { print(scale(3)); }",
        );
        let c = emit_c(&program).unwrap();
        assert!(c.contains("((double)(INT64_C(3)))"));
    }

    #[test]
    fn emits_string_runtime_support() {
        let program = compile_source(
            "fn greet(name: string) -> string { return \"Hello \" + name; } fn main() { print(greet(\"Genix\")); }",
        );
        let c = emit_c(&program).unwrap();
        assert!(c.contains("gb_concat"));
        assert!(c.contains("printf(\"%s\\n\""));
    }
}
