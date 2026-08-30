use std::collections::HashMap;

use crate::ast::{self, BinaryOp, Pattern, Type, UnaryOp};

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let { name: String, value: Expr, mutable: bool, ty: Type },
    Assign { name: String, value: Expr },
    Print(Expr),
    Expr(Expr),
    Return(Option<Expr>),
    If { condition: Expr, then_branch: Vec<Stmt>, else_branch: Option<Vec<Stmt>> },
    While { condition: Expr, body: Vec<Stmt> },
    Match { value: Expr, arms: Vec<MatchArm> },
    Block(Vec<Stmt>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub binding_ty: Option<Type>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub ty: Type,
    pub kind: ExprKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Integer(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Variable(String),
    Call { callee: String, arguments: Vec<Expr> },
    Some(Box<Expr>),
    None,
    Ok(Box<Expr>),
    Err(Box<Expr>),
    Try(Box<Expr>),
    Cast { expr: Box<Expr>, to: Type },
    Unary { op: UnaryOp, expr: Box<Expr> },
    Binary { left: Box<Expr>, op: BinaryOp, right: Box<Expr> },
}

#[derive(Debug, Clone)]
struct Signature {
    params: Vec<Type>,
    return_type: Type,
}

pub fn lower(program: &ast::Program) -> Result<Program, String> {
    let mut signatures = HashMap::new();
    for function in &program.functions {
        if signatures.insert(
            function.name.clone(),
            Signature {
                params: function.params.iter().map(|param| param.ty).collect(),
                return_type: function.return_type,
            },
        ).is_some() {
            return Err(format!("IR lowering: duplicate function '{}'", function.name));
        }
    }

    let mut lowerer = Lowerer {
        signatures,
        scopes: Vec::new(),
        return_type: Type::Void,
    };
    let functions = program.functions.iter()
        .map(|function| lowerer.lower_function(function))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Program { functions })
}

struct Lowerer {
    signatures: HashMap<String, Signature>,
    scopes: Vec<HashMap<String, Type>>,
    return_type: Type,
}

impl Lowerer {
    fn lower_function(&mut self, function: &ast::Function) -> Result<Function, String> {
        self.scopes = vec![HashMap::new()];
        self.return_type = function.return_type;
        let params = function.params.iter().map(|param| {
            self.scopes[0].insert(param.name.clone(), param.ty);
            Param { name: param.name.clone(), ty: param.ty }
        }).collect();
        let body = self.lower_statements(&function.body)?;
        self.scopes.clear();
        Ok(Function {
            name: function.name.clone(),
            params,
            return_type: function.return_type,
            body,
        })
    }

    fn lower_statements(&mut self, statements: &[ast::Stmt]) -> Result<Vec<Stmt>, String> {
        statements.iter().map(|statement| self.lower_statement(statement)).collect()
    }

    fn lower_block(&mut self, statements: &[ast::Stmt]) -> Result<Vec<Stmt>, String> {
        self.scopes.push(HashMap::new());
        let result = self.lower_statements(statements);
        self.scopes.pop();
        result
    }

    fn lower_statement(&mut self, statement: &ast::Stmt) -> Result<Stmt, String> {
        match statement {
            ast::Stmt::Let { name, value, mutable, annotation } => {
                let value = if let Some(expected) = annotation {
                    self.lower_expr_expected(value, Some(*expected))?
                } else {
                    self.lower_expr(value)?
                };
                let ty = annotation.unwrap_or(value.ty);
                let value = coerce(value, ty)?;
                self.current_scope_mut()?.insert(name.clone(), ty);
                Ok(Stmt::Let { name: name.clone(), value, mutable: *mutable, ty })
            }
            ast::Stmt::Assign { name, value } => {
                let ty = self.lookup(name).ok_or_else(|| format!("IR lowering: undefined variable '{name}'"))?;
                let value = coerce(self.lower_expr_expected(value, Some(ty))?, ty)?;
                Ok(Stmt::Assign { name: name.clone(), value })
            }
            ast::Stmt::Print(expr) => Ok(Stmt::Print(self.lower_expr(expr)?)),
            ast::Stmt::Expr(expr) => Ok(Stmt::Expr(self.lower_expr(expr)?)),
            ast::Stmt::Return(Some(expr)) => {
                let value = coerce(self.lower_expr_expected(expr, Some(self.return_type))?, self.return_type)?;
                Ok(Stmt::Return(Some(value)))
            }
            ast::Stmt::Return(None) => Ok(Stmt::Return(None)),
            ast::Stmt::If { condition, then_branch, else_branch } => Ok(Stmt::If {
                condition: self.lower_expr(condition)?,
                then_branch: self.lower_block(then_branch)?,
                else_branch: else_branch.as_ref().map(|branch| self.lower_block(branch)).transpose()?,
            }),
            ast::Stmt::While { condition, body } => Ok(Stmt::While {
                condition: self.lower_expr(condition)?,
                body: self.lower_block(body)?,
            }),
            ast::Stmt::Match { value, arms } => {
                let value = self.lower_expr(value)?;
                let matched_ty = value.ty;
                let mut lowered_arms = Vec::new();
                for arm in arms {
                    let binding = match (&arm.pattern, matched_ty) {
                        (Pattern::Some(name), ty) => Some((name.clone(), ty.option_inner().ok_or_else(|| "IR lowering: Some pattern on non-Option".to_string())?)),
                        (Pattern::None, ty) if ty.option_inner().is_some() => None,
                        (Pattern::Ok(name), ty) => Some((name.clone(), ty.result_ok().ok_or_else(|| "IR lowering: Ok pattern on non-Result".to_string())?)),
                        (Pattern::Err(name), ty) if ty.result_ok().is_some() => Some((name.clone(), Type::String)),
                        _ => return Err("IR lowering: invalid match pattern".into()),
                    };
                    self.scopes.push(HashMap::new());
                    if let Some((name, ty)) = &binding {
                        self.scopes.last_mut().unwrap().insert(name.clone(), *ty);
                    }
                    let body = self.lower_statements(&arm.body)?;
                    self.scopes.pop();
                    lowered_arms.push(MatchArm {
                        pattern: arm.pattern.clone(),
                        binding_ty: binding.map(|(_, ty)| ty),
                        body,
                    });
                }
                Ok(Stmt::Match { value, arms: lowered_arms })
            }
            ast::Stmt::Block(body) => Ok(Stmt::Block(self.lower_block(body)?)),
        }
    }

    fn lower_expr(&mut self, expr: &ast::Expr) -> Result<Expr, String> {
        self.lower_expr_expected(expr, None)
    }

    fn lower_expr_expected(&mut self, expr: &ast::Expr, expected: Option<Type>) -> Result<Expr, String> {
        match expr {
            ast::Expr::Integer(value) => Ok(Expr { ty: Type::Int, kind: ExprKind::Integer(*value) }),
            ast::Expr::Float(value) => Ok(Expr { ty: Type::Float, kind: ExprKind::Float(*value) }),
            ast::Expr::Bool(value) => Ok(Expr { ty: Type::Bool, kind: ExprKind::Bool(*value) }),
            ast::Expr::String(value) => Ok(Expr { ty: Type::String, kind: ExprKind::String(value.clone()) }),
            ast::Expr::Variable(name) => Ok(Expr {
                ty: self.lookup(name).ok_or_else(|| format!("IR lowering: undefined variable '{name}'"))?,
                kind: ExprKind::Variable(name.clone()),
            }),
            ast::Expr::Call { callee, arguments } => {
                let signature = self.signatures.get(callee).cloned()
                    .ok_or_else(|| format!("IR lowering: undefined function '{callee}'"))?;
                let arguments = arguments.iter().zip(signature.params.iter())
                    .map(|(argument, expected)| {
                        let lowered = self.lower_expr_expected(argument, Some(*expected))?;
                        coerce(lowered, *expected)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Expr {
                    ty: signature.return_type,
                    kind: ExprKind::Call { callee: callee.clone(), arguments },
                })
            }
            ast::Expr::Some(value) => {
                let ty = if let Some(expected_ty) = expected.filter(|ty| ty.option_inner().is_some()) {
                    expected_ty
                } else {
                    let probe = self.lower_expr(value)?;
                    Type::option(probe.ty).ok_or_else(|| "IR lowering: unsupported Some payload".to_string())?
                };
                let inner_ty = ty.option_inner().unwrap();
                let value = coerce(self.lower_expr_expected(value, Some(inner_ty))?, inner_ty)?;
                Ok(Expr { ty, kind: ExprKind::Some(Box::new(value)) })
            }
            ast::Expr::None => {
                let ty = expected.filter(|ty| ty.option_inner().is_some())
                    .ok_or_else(|| "IR lowering: None requires Option context".to_string())?;
                Ok(Expr { ty, kind: ExprKind::None })
            }
            ast::Expr::Ok(value) => {
                let ty = if let Some(expected_ty) = expected.filter(|ty| ty.result_ok().is_some()) {
                    expected_ty
                } else {
                    let probe = self.lower_expr(value)?;
                    Type::result(probe.ty, Type::String).ok_or_else(|| "IR lowering: unsupported Ok payload".to_string())?
                };
                let ok_ty = ty.result_ok().unwrap();
                let value = coerce(self.lower_expr_expected(value, Some(ok_ty))?, ok_ty)?;
                Ok(Expr { ty, kind: ExprKind::Ok(Box::new(value)) })
            }
            ast::Expr::Err(error) => {
                let ty = expected.filter(|ty| ty.result_ok().is_some())
                    .ok_or_else(|| "IR lowering: Err requires Result context".to_string())?;
                let error = coerce(self.lower_expr_expected(error, Some(Type::String))?, Type::String)?;
                Ok(Expr { ty, kind: ExprKind::Err(Box::new(error)) })
            }
            ast::Expr::Try(inner) => {
                let inner = self.lower_expr(inner)?;
                let ty = inner.ty.result_ok().ok_or_else(|| "IR lowering: '?' requires Result".to_string())?;
                Ok(Expr { ty, kind: ExprKind::Try(Box::new(inner)) })
            }
            ast::Expr::Unary { op, expr } => {
                let expr = self.lower_expr(expr)?;
                let ty = match op { UnaryOp::Negate => expr.ty, UnaryOp::Not => Type::Bool };
                Ok(Expr { ty, kind: ExprKind::Unary { op: *op, expr: Box::new(expr) } })
            }
            ast::Expr::Binary { left, op, right } => {
                let left = self.lower_expr(left)?;
                let right = self.lower_expr(right)?;
                self.lower_binary(left, *op, right)
            }
        }
    }

    fn lower_binary(&self, mut left: Expr, op: BinaryOp, mut right: Expr) -> Result<Expr, String> {
        let ty = match op {
            BinaryOp::Add if left.ty == Type::String && right.ty == Type::String => Type::String,
            BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Divide => {
                let result = numeric_result(left.ty, right.ty)?;
                if result == Type::Float {
                    left = coerce(left, Type::Float)?;
                    right = coerce(right, Type::Float)?;
                }
                result
            }
            BinaryOp::Equal | BinaryOp::NotEqual | BinaryOp::Less | BinaryOp::LessEqual
            | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                if is_numeric(left.ty) && is_numeric(right.ty) && (left.ty == Type::Float || right.ty == Type::Float) {
                    left = coerce(left, Type::Float)?;
                    right = coerce(right, Type::Float)?;
                }
                Type::Bool
            }
            BinaryOp::And | BinaryOp::Or => Type::Bool,
        };
        Ok(Expr { ty, kind: ExprKind::Binary { left: Box::new(left), op, right: Box::new(right) } })
    }

    fn current_scope_mut(&mut self) -> Result<&mut HashMap<String, Type>, String> {
        self.scopes.last_mut().ok_or_else(|| "IR lowering: no active scope".to_string())
    }

    fn lookup(&self, name: &str) -> Option<Type> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name).copied())
    }
}

fn coerce(expr: Expr, expected: Type) -> Result<Expr, String> {
    if expr.ty == expected { return Ok(expr); }
    if expected == Type::Float && expr.ty == Type::Int {
        return Ok(Expr { ty: Type::Float, kind: ExprKind::Cast { expr: Box::new(expr), to: Type::Float } });
    }
    Err(format!("IR lowering: cannot convert {} to {expected}", expr.ty))
}

fn is_numeric(ty: Type) -> bool { matches!(ty, Type::Int | Type::Float) }

fn numeric_result(left: Type, right: Type) -> Result<Type, String> {
    if !is_numeric(left) || !is_numeric(right) {
        return Err(format!("IR lowering: numeric operation requires int/float, found {left} and {right}"));
    }
    Ok(if left == Type::Float || right == Type::Float { Type::Float } else { Type::Int })
}

pub fn format(program: &Program) -> String {
    let mut out = String::new();
    for (index, function) in program.functions.iter().enumerate() {
        if index > 0 { out.push('\n'); }
        let params = function.params.iter()
            .map(|param| format!("{}: {}", param.name, param.ty))
            .collect::<Vec<_>>().join(", ");
        out.push_str(&format!("fn {}({}) -> {} {{\n", function.name, params, function.return_type));
        format_statements(&function.body, 1, &mut out);
        out.push_str("}\n");
    }
    out
}

fn format_statements(statements: &[Stmt], indent: usize, out: &mut String) {
    for statement in statements {
        let pad = "    ".repeat(indent);
        match statement {
            Stmt::Let { name, value, mutable, ty } => out.push_str(&format!(
                "{pad}{} {}: {} = {};\n",
                if *mutable { "mut" } else { "let" }, name, ty, format_expr(value)
            )),
            Stmt::Assign { name, value } => out.push_str(&format!("{pad}{name} = {};\n", format_expr(value))),
            Stmt::Print(expr) => out.push_str(&format!("{pad}print({});\n", format_expr(expr))),
            Stmt::Expr(expr) => out.push_str(&format!("{pad}{};\n", format_expr(expr))),
            Stmt::Return(Some(expr)) => out.push_str(&format!("{pad}return {};\n", format_expr(expr))),
            Stmt::Return(None) => out.push_str(&format!("{pad}return;\n")),
            Stmt::If { condition, then_branch, else_branch } => {
                out.push_str(&format!("{pad}if {} {{\n", format_expr(condition)));
                format_statements(then_branch, indent + 1, out);
                if let Some(branch) = else_branch {
                    out.push_str(&format!("{pad}}} else {{\n"));
                    format_statements(branch, indent + 1, out);
                }
                out.push_str(&format!("{pad}}}\n"));
            }
            Stmt::While { condition, body } => {
                out.push_str(&format!("{pad}while {} {{\n", format_expr(condition)));
                format_statements(body, indent + 1, out);
                out.push_str(&format!("{pad}}}\n"));
            }
            Stmt::Match { value, arms } => {
                out.push_str(&format!("{pad}match {} {{\n", format_expr(value)));
                for arm in arms {
                    out.push_str(&format!("{pad}    {} => {{\n", format_pattern(&arm.pattern)));
                    format_statements(&arm.body, indent + 2, out);
                    out.push_str(&format!("{pad}    }}\n"));
                }
                out.push_str(&format!("{pad}}}\n"));
            }
            Stmt::Block(body) => {
                out.push_str(&format!("{pad}{{\n"));
                format_statements(body, indent + 1, out);
                out.push_str(&format!("{pad}}}\n"));
            }
        }
    }
}

fn format_pattern(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Some(name) => format!("Some({name})"),
        Pattern::None => "None".into(),
        Pattern::Ok(name) => format!("Ok({name})"),
        Pattern::Err(name) => format!("Err({name})"),
    }
}

fn format_expr(expr: &Expr) -> String {
    let value = match &expr.kind {
        ExprKind::Integer(value) => value.to_string(),
        ExprKind::Float(value) => value.to_string(),
        ExprKind::Bool(value) => value.to_string(),
        ExprKind::String(value) => format!("\"{}\"", value.replace('"', "\\\"")),
        ExprKind::Variable(name) => name.clone(),
        ExprKind::Call { callee, arguments } => format!(
            "{}({})", callee, arguments.iter().map(format_expr).collect::<Vec<_>>().join(", ")
        ),
        ExprKind::Some(value) => format!("Some({})", format_expr(value)),
        ExprKind::None => "None".into(),
        ExprKind::Ok(value) => format!("Ok({})", format_expr(value)),
        ExprKind::Err(error) => format!("Err({})", format_expr(error)),
        ExprKind::Try(value) => format!("{}?", format_expr(value)),
        ExprKind::Cast { expr, to } => format!("cast<{}>({})", to, format_expr(expr)),
        ExprKind::Unary { op, expr } => format!("{}{}", match op { UnaryOp::Negate => "-", UnaryOp::Not => "!" }, format_expr(expr)),
        ExprKind::Binary { left, op, right } => format!("({} {} {})", format_expr(left), binary_symbol(*op), format_expr(right)),
    };
    format!("{value}:{}", expr.ty)
}

fn binary_symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+", BinaryOp::Subtract => "-", BinaryOp::Multiply => "*", BinaryOp::Divide => "/",
        BinaryOp::Equal => "==", BinaryOp::NotEqual => "!=", BinaryOp::Less => "<", BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">", BinaryOp::GreaterEqual => ">=", BinaryOp::And => "&&", BinaryOp::Or => "||",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, parser, typechecker};

    fn lower_source(source: &str) -> Program {
        let ast = parser::parse(lexer::lex(source).unwrap()).unwrap();
        typechecker::check(&ast).unwrap();
        lower(&ast).unwrap()
    }

    #[test]
    fn lowers_result_try_with_types() {
        let ir = lower_source("fn load() -> Result<string,string> { return Ok(\"x\"); } fn wrap() -> Result<string,string> { let x: string = load()?; return Ok(x); } fn main() {}");
        match &ir.functions[1].body[0] {
            Stmt::Let { value, ty, .. } => {
                assert_eq!(*ty, Type::String);
                assert!(matches!(value.kind, ExprKind::Try(_)));
            }
            _ => panic!("expected try let"),
        }
    }

    #[test]
    fn formats_sum_types() {
        let ir = lower_source("fn main() { let x: Option<string> = Some(\"Genix\"); match x { Some(v) => { print(v); } None => { print(\"none\"); } } }");
        let text = format(&ir);
        assert!(text.contains("Option<string>"));
        assert!(text.contains("match"));
    }
}
