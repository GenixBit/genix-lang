use std::collections::HashMap;

use crate::ast::{
    BinaryOp, Expr, Function, MatchArm, Pattern, Program, Stmt, StmtKind, Type, UnaryOp,
};
use crate::diagnostics::Diagnostic;

#[derive(Debug, Clone)]
struct Signature {
    params: Vec<Type>,
    return_type: Type,
}

#[derive(Debug, Clone, Copy)]
struct BindingInfo {
    ty: Type,
    mutable: bool,
}

pub fn check(program: &Program) -> Result<(), Diagnostic> {
    let signatures = collect_signatures(program)?;
    validate_main(program, &signatures)?;

    for function in &program.functions {
        let mut checker = Checker::new(
            &signatures,
            function.return_type,
            function.source_name.clone(),
        );
        checker.check_function(function)?;

        if function.return_type != Type::Void && !block_definitely_returns(&function.body) {
            return Err(
                Diagnostic::type_error(
                    "E0204",
                    format!(
                        "function '{}' does not return {} on every path",
                        function.name, function.return_type
                    ),
                )
                .with_label("a return value is required on every path")
                .with_help("add a return statement to every control-flow path")
                .with_location(function.source_name.clone(), function.span),
            );
        }
    }
    Ok(())
}

fn collect_signatures(program: &Program) -> Result<HashMap<String, Signature>, Diagnostic> {
    let mut signatures = HashMap::new();
    for function in &program.functions {
        if signatures.contains_key(&function.name) {
            return Err(
                Diagnostic::type_error(
                    "E0209",
                    format!("function '{}' is defined more than once", function.name),
                )
                .with_label("duplicate function definition")
                .with_help("rename or remove one of the duplicate functions")
                .with_location(function.source_name.clone(), function.span),
            );
        }
        signatures.insert(
            function.name.clone(),
            Signature {
                params: function.params.iter().map(|param| param.ty).collect(),
                return_type: function.return_type,
            },
        );
    }
    Ok(signatures)
}

fn validate_main(
    program: &Program,
    signatures: &HashMap<String, Signature>,
) -> Result<(), Diagnostic> {
    let Some(main) = signatures.get("main") else {
        return Err(
            Diagnostic::type_error("E0210", "Genix program must define fn main()")
                .with_help("add `fn main() { ... }` as the program entry point"),
        );
    };
    let main_function = program.functions.iter().find(|function| function.name == "main");

    if !main.params.is_empty() {
        let mut diagnostic = Diagnostic::type_error(
            "E0210",
            "fn main() cannot take parameters in Genix v0.1",
        )
        .with_help("remove the parameters from fn main()");
        if let Some(function) = main_function {
            diagnostic = diagnostic.with_location(function.source_name.clone(), function.span);
        }
        return Err(diagnostic);
    }
    if main.return_type != Type::Void {
        let mut diagnostic = Diagnostic::type_error(
            "E0210",
            "fn main() cannot declare a return value in Genix v0.1",
        )
        .with_help("remove the return type from fn main()");
        if let Some(function) = main_function {
            diagnostic = diagnostic.with_location(function.source_name.clone(), function.span);
        }
        return Err(diagnostic);
    }
    Ok(())
}

struct Checker<'a> {
    signatures: &'a HashMap<String, Signature>,
    scopes: Vec<HashMap<String, BindingInfo>>,
    return_type: Type,
    source_name: String,
}

impl<'a> Checker<'a> {
    fn new(
        signatures: &'a HashMap<String, Signature>,
        return_type: Type,
        source_name: String,
    ) -> Self {
        Self {
            signatures,
            scopes: vec![HashMap::new()],
            return_type,
            source_name,
        }
    }

    fn check_function(&mut self, function: &Function) -> Result<(), Diagnostic> {
        for param in &function.params {
            if param.ty == Type::Void {
                return Err(
                    Diagnostic::type_error(
                        "E0208",
                        format!("parameter '{}' cannot have type void", param.name),
                    )
                    .with_label("invalid parameter type")
                    .with_help("choose a value type for this parameter")
                    .with_location(function.source_name.clone(), function.span),
                );
            }
            let scope = self.scopes.last_mut().unwrap();
            if scope.contains_key(&param.name) {
                return Err(
                    Diagnostic::type_error(
                        "E0209",
                        format!("duplicate parameter '{}'", param.name),
                    )
                    .with_label("parameter name is already used")
                    .with_help("rename one of the parameters")
                    .with_location(function.source_name.clone(), function.span),
                );
            }
            scope.insert(
                param.name.clone(),
                BindingInfo {
                    ty: param.ty,
                    mutable: false,
                },
            );
        }
        for statement in &function.body {
            self.check_statement(statement)?;
        }
        Ok(())
    }

    fn check_statement(&mut self, statement: &Stmt) -> Result<(), Diagnostic> {
        self.check_statement_kind(&statement.kind)
            .map_err(|diagnostic| diagnostic.with_location(self.source_name.clone(), statement.span))
    }

    fn check_statement_kind(&mut self, statement: &StmtKind) -> Result<(), Diagnostic> {
        match statement {
            StmtKind::Let {
                name,
                value,
                mutable,
                annotation,
            } => {
                self.ensure_try_position(value, true, "variable initializer")?;
                let actual = self.expression_type_expected(value, *annotation)?;
                if actual == Type::Void {
                    return Err(
                        Diagnostic::type_error(
                            "E0208",
                            format!("variable '{name}' cannot store a void value"),
                        )
                        .with_label("void values cannot be stored"),
                    );
                }
                let ty = if let Some(expected) = annotation {
                    if *expected == Type::Void {
                        return Err(
                            Diagnostic::type_error(
                                "E0208",
                                format!("variable '{name}' cannot have type void"),
                            )
                            .with_label("invalid variable type"),
                        );
                    }
                    self.require_compatible(
                        *expected,
                        actual,
                        &format!("initializer for '{name}'"),
                    )?;
                    *expected
                } else {
                    actual
                };
                let scope = self.scopes.last_mut().unwrap();
                if scope.contains_key(name) {
                    return Err(
                        Diagnostic::type_error(
                            "E0209",
                            format!("variable '{name}' is already declared in this scope"),
                        )
                        .with_label("duplicate variable declaration")
                        .with_help("choose a different name or assign to the existing mutable variable"),
                    );
                }
                scope.insert(
                    name.clone(),
                    BindingInfo {
                        ty,
                        mutable: *mutable,
                    },
                );
                Ok(())
            }
            StmtKind::Assign { name, value } => {
                self.ensure_try_position(value, true, "assignment")?;
                let binding = self.lookup(name).ok_or_else(|| {
                    Diagnostic::type_error("E0202", format!("undefined variable '{name}'"))
                        .with_label("name is not defined in this scope")
                        .with_help("declare the variable before assigning to it")
                })?;
                if !binding.mutable {
                    return Err(
                        Diagnostic::type_error(
                            "E0203",
                            format!("cannot assign to immutable variable '{name}'"),
                        )
                        .with_label("this variable was declared immutable")
                        .with_help(format!("declare it with `mut {name} = ...` if mutation is intended")),
                    );
                }
                let actual = self.expression_type_expected(value, Some(binding.ty))?;
                self.require_compatible(binding.ty, actual, &format!("assignment to '{name}'"))
            }
            StmtKind::Print(expr) => {
                self.ensure_try_position(expr, false, "print argument")?;
                let ty = self.expression_type(expr)?;
                if ty == Type::Void {
                    return Err(
                        Diagnostic::type_error("E0208", "print() cannot print a void value")
                            .with_label("print requires a value"),
                    );
                }
                if ty.option_inner().is_some() || ty.result_ok().is_some() {
                    return Err(
                        Diagnostic::type_error(
                            "E0208",
                            format!("print() cannot directly print {ty}"),
                        )
                        .with_label("wrapped value must be handled first")
                        .with_help("use match to unwrap Option or Result before printing"),
                    );
                }
                Ok(())
            }
            StmtKind::Expr(expr) => {
                self.ensure_try_position(expr, true, "expression statement")?;
                if !matches!(expr, Expr::Call { .. } | Expr::Try(_)) {
                    return Err(
                        Diagnostic::type_error(
                            "E0208",
                            "only function calls can be expression statements",
                        )
                        .with_help("assign or return the expression value instead"),
                    );
                }
                self.expression_type(expr)?;
                Ok(())
            }
            StmtKind::Return(value) => match (self.return_type, value) {
                (Type::Void, None) => Ok(()),
                (Type::Void, Some(_)) => Err(
                    Diagnostic::type_error("E0204", "void function cannot return a value")
                        .with_label("unexpected return value")
                        .with_help("use `return;` or remove the return statement"),
                ),
                (expected, None) => Err(
                    Diagnostic::type_error(
                        "E0204",
                        format!("expected return value of type {expected}"),
                    )
                    .with_label("return value is missing"),
                ),
                (expected, Some(expr)) => {
                    self.ensure_try_position(expr, false, "return value")?;
                    let actual = self.expression_type_expected(expr, Some(expected))?;
                    self.require_compatible(expected, actual, "return value")
                }
            },
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.ensure_try_position(condition, false, "if condition")?;
                let actual = self.expression_type(condition)?;
                self.require_exact(Type::Bool, actual, "if condition")?;
                self.check_block(then_branch)?;
                if let Some(branch) = else_branch {
                    self.check_block(branch)?;
                }
                Ok(())
            }
            StmtKind::While { condition, body } => {
                self.ensure_try_position(condition, false, "while condition")?;
                let actual = self.expression_type(condition)?;
                self.require_exact(Type::Bool, actual, "while condition")?;
                self.check_block(body)
            }
            StmtKind::Match { value, arms } => self.check_match(value, arms),
            StmtKind::Block(body) => self.check_block(body),
        }
    }

    fn check_match(&mut self, value: &Expr, arms: &[MatchArm]) -> Result<(), Diagnostic> {
        self.ensure_try_position(value, false, "match value")?;
        let ty = self.expression_type(value)?;
        let is_option = ty.option_inner().is_some();
        let is_result = ty.result_ok().is_some();
        if !is_option && !is_result {
            return Err(
                Diagnostic::type_error(
                    "E0205",
                    format!("match currently supports Option and Result, found {ty}"),
                )
                .with_label("unsupported match value")
                .with_help("match an Option<T> or Result<T,string> value"),
            );
        }

        let mut first_seen = false;
        let mut second_seen = false;
        for arm in arms {
            let binding = match (&arm.pattern, ty) {
                (Pattern::Some(name), option) if option.option_inner().is_some() => {
                    if first_seen {
                        return Err(
                            Diagnostic::type_error("E0205", "duplicate Some match arm")
                                .with_help("keep exactly one Some(...) arm"),
                        );
                    }
                    first_seen = true;
                    Some((name.as_str(), option.option_inner().unwrap()))
                }
                (Pattern::None, option) if option.option_inner().is_some() => {
                    if second_seen {
                        return Err(
                            Diagnostic::type_error("E0205", "duplicate None match arm")
                                .with_help("keep exactly one None arm"),
                        );
                    }
                    second_seen = true;
                    None
                }
                (Pattern::Ok(name), result) if result.result_ok().is_some() => {
                    if first_seen {
                        return Err(
                            Diagnostic::type_error("E0205", "duplicate Ok match arm")
                                .with_help("keep exactly one Ok(...) arm"),
                        );
                    }
                    first_seen = true;
                    Some((name.as_str(), result.result_ok().unwrap()))
                }
                (Pattern::Err(name), result) if result.result_ok().is_some() => {
                    if second_seen {
                        return Err(
                            Diagnostic::type_error("E0205", "duplicate Err match arm")
                                .with_help("keep exactly one Err(...) arm"),
                        );
                    }
                    second_seen = true;
                    Some((name.as_str(), Type::String))
                }
                (pattern, _) => {
                    return Err(
                        Diagnostic::type_error(
                            "E0205",
                            format!("pattern {pattern:?} is not valid for {ty}"),
                        )
                        .with_help("use Some/None for Option and Ok/Err for Result"),
                    )
                }
            };

            self.scopes.push(HashMap::new());
            if let Some((name, binding_ty)) = binding {
                self.scopes.last_mut().unwrap().insert(
                    name.to_string(),
                    BindingInfo {
                        ty: binding_ty,
                        mutable: false,
                    },
                );
            }
            let result: Result<(), Diagnostic> = (|| {
                for statement in &arm.body {
                    self.check_statement(statement)?;
                }
                Ok(())
            })();
            self.scopes.pop();
            result?;
        }

        if is_option && !(first_seen && second_seen) {
            return Err(
                Diagnostic::type_error(
                    "E0205",
                    "Option match must handle both Some(...) and None",
                )
                .with_label("non-exhaustive Option match")
                .with_help("add the missing Some(...) or None arm"),
            );
        }
        if is_result && !(first_seen && second_seen) {
            return Err(
                Diagnostic::type_error(
                    "E0205",
                    "Result match must handle both Ok(...) and Err(...)",
                )
                .with_label("non-exhaustive Result match")
                .with_help("add the missing Ok(...) or Err(...) arm"),
            );
        }
        Ok(())
    }

    fn check_block(&mut self, body: &[Stmt]) -> Result<(), Diagnostic> {
        self.scopes.push(HashMap::new());
        let result: Result<(), Diagnostic> = (|| {
            for statement in body {
                self.check_statement(statement)?;
            }
            Ok(())
        })();
        self.scopes.pop();
        result
    }

    fn expression_type(&self, expr: &Expr) -> Result<Type, Diagnostic> {
        self.expression_type_expected(expr, None)
    }

    fn expression_type_expected(
        &self,
        expr: &Expr,
        expected: Option<Type>,
    ) -> Result<Type, Diagnostic> {
        match expr {
            Expr::Integer(_) => Ok(Type::Int),
            Expr::Float(_) => Ok(Type::Float),
            Expr::Bool(_) => Ok(Type::Bool),
            Expr::String(_) => Ok(Type::String),
            Expr::Variable(name) => self.lookup(name).map(|binding| binding.ty).ok_or_else(|| {
                Diagnostic::type_error("E0202", format!("undefined variable '{name}'"))
                    .with_label("name is not defined in this scope")
                    .with_help("declare the variable before using it")
            }),
            Expr::Call { callee, arguments } => {
                let signature = self.signatures.get(callee).cloned().ok_or_else(|| {
                    Diagnostic::type_error("E0202", format!("undefined function '{callee}'"))
                        .with_label("function is not defined")
                        .with_help("check the function name or import the module that defines it")
                })?;
                if arguments.len() != signature.params.len() {
                    return Err(
                        Diagnostic::type_error(
                            "E0207",
                            format!(
                                "function '{callee}' expects {} argument(s), found {}",
                                signature.params.len(),
                                arguments.len()
                            ),
                        )
                        .with_label("wrong number of arguments")
                        .with_help("pass the number of arguments declared by the function"),
                    );
                }
                for (index, (argument, param_ty)) in
                    arguments.iter().zip(signature.params.iter()).enumerate()
                {
                    let actual = self.expression_type_expected(argument, Some(*param_ty))?;
                    self.require_compatible(
                        *param_ty,
                        actual,
                        &format!("argument {} for function '{callee}'", index + 1),
                    )?;
                }
                Ok(signature.return_type)
            }
            Expr::Some(value) => {
                if let Some(option_ty) = expected.filter(|ty| ty.option_inner().is_some()) {
                    let inner = option_ty.option_inner().unwrap();
                    let actual = self.expression_type_expected(value, Some(inner))?;
                    self.require_compatible(inner, actual, "Some value")?;
                    Ok(option_ty)
                } else {
                    let inner = self.expression_type(value)?;
                    Type::option(inner).ok_or_else(|| {
                        Diagnostic::type_error(
                            "E0208",
                            format!("Some({inner}) is not supported in Genix v0.1"),
                        )
                        .with_help("use a primitive int, float, bool, or string payload")
                    })
                }
            }
            Expr::None => expected
                .filter(|ty| ty.option_inner().is_some())
                .ok_or_else(|| {
                    Diagnostic::type_error(
                        "E0208",
                        "None requires an Option<T> type annotation or return context",
                    )
                    .with_help("add an Option<T> annotation, for example `let value: Option<string> = None;`")
                }),
            Expr::Ok(value) => {
                if let Some(result_ty) = expected.filter(|ty| ty.result_ok().is_some()) {
                    let ok_ty = result_ty.result_ok().unwrap();
                    let actual = self.expression_type_expected(value, Some(ok_ty))?;
                    self.require_compatible(ok_ty, actual, "Ok value")?;
                    Ok(result_ty)
                } else {
                    let ok_ty = self.expression_type(value)?;
                    Type::result(ok_ty, Type::String).ok_or_else(|| {
                        Diagnostic::type_error(
                            "E0208",
                            format!("Ok({ok_ty}) is not supported in Genix v0.1"),
                        )
                    })
                }
            }
            Expr::Err(error) => {
                let error_ty = self.expression_type_expected(error, Some(Type::String))?;
                self.require_exact(Type::String, error_ty, "Err value")?;
                expected
                    .filter(|ty| ty.result_ok().is_some())
                    .ok_or_else(|| {
                        Diagnostic::type_error(
                            "E0208",
                            "Err(...) requires a Result<T,string> type annotation or return context",
                        )
                        .with_help("use Err(...) where a Result<T,string> type is expected")
                    })
            }
            Expr::Try(inner) => {
                let result_ty = self.expression_type(inner)?;
                let Some(ok_ty) = result_ty.result_ok() else {
                    return Err(
                        Diagnostic::type_error(
                            "E0206",
                            format!("'?' requires Result<T,string>, found {result_ty}"),
                        )
                        .with_label("this expression is not a Result")
                        .with_help("remove '?' or make the expression return Result<T,string>"),
                    );
                };
                if !self.return_type.is_result() {
                    return Err(
                        Diagnostic::type_error(
                            "E0206",
                            format!(
                                "'?' can only be used inside a function returning Result<T,string>; current return type is {}",
                                self.return_type
                            ),
                        )
                        .with_label("error cannot be propagated from this function")
                        .with_help("change the function return type to Result<T,string> or handle the Result with match"),
                    );
                }
                Ok(ok_ty)
            }
            Expr::Unary { op, expr } => {
                let ty = self.expression_type(expr)?;
                match op {
                    UnaryOp::Negate if is_numeric(ty) => Ok(ty),
                    UnaryOp::Negate => Err(
                        Diagnostic::type_error(
                            "E0201",
                            format!("unary '-' requires a number, found {ty}"),
                        )
                        .with_label("type mismatch"),
                    ),
                    UnaryOp::Not if ty == Type::Bool => Ok(Type::Bool),
                    UnaryOp::Not => Err(
                        Diagnostic::type_error(
                            "E0201",
                            format!("operator '!' requires bool, found {ty}"),
                        )
                        .with_label("type mismatch"),
                    ),
                }
            }
            Expr::Binary { left, op, right } => {
                let left_ty = self.expression_type(left)?;
                let right_ty = self.expression_type(right)?;
                self.binary_type(left_ty, *op, right_ty)
            }
        }
    }

    fn binary_type(&self, left: Type, op: BinaryOp, right: Type) -> Result<Type, Diagnostic> {
        match op {
            BinaryOp::Add if left == Type::String && right == Type::String => Ok(Type::String),
            BinaryOp::Add => numeric_result(left, right, "+"),
            BinaryOp::Subtract => numeric_result(left, right, "-"),
            BinaryOp::Multiply => numeric_result(left, right, "*"),
            BinaryOp::Divide => numeric_result(left, right, "/"),
            BinaryOp::Equal | BinaryOp::NotEqual => {
                if (left == right && is_plain_value(left)) || (is_numeric(left) && is_numeric(right)) {
                    Ok(Type::Bool)
                } else {
                    Err(
                        Diagnostic::type_error(
                            "E0201",
                            format!(
                                "equality comparison requires compatible primitive types, found {left} and {right}"
                            ),
                        )
                        .with_label("incompatible operands"),
                    )
                }
            }
            BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual => {
                if is_numeric(left) && is_numeric(right) {
                    Ok(Type::Bool)
                } else {
                    Err(
                        Diagnostic::type_error(
                            "E0201",
                            format!("comparison requires numbers, found {left} and {right}"),
                        )
                        .with_label("non-numeric comparison"),
                    )
                }
            }
            BinaryOp::And | BinaryOp::Or => {
                if left == Type::Bool && right == Type::Bool {
                    Ok(Type::Bool)
                } else {
                    Err(
                        Diagnostic::type_error(
                            "E0201",
                            format!(
                                "logical operator requires bool operands, found {left} and {right}"
                            ),
                        )
                        .with_label("type mismatch"),
                    )
                }
            }
        }
    }

    fn ensure_try_position(
        &self,
        expr: &Expr,
        allow_root: bool,
        context: &str,
    ) -> Result<(), Diagnostic> {
        if contains_try(expr) && !(allow_root && matches!(expr, Expr::Try(_))) {
            return Err(
                Diagnostic::type_error(
                    "E0206",
                    format!(
                        "'?' is currently supported only as the complete value of a variable initializer, assignment, or call statement; not inside {context}"
                    ),
                )
                .with_label("unsupported '?' position")
                .with_help("bind the Result value with '?' first, then use the unwrapped value in the larger expression"),
            );
        }
        Ok(())
    }

    fn lookup(&self, name: &str) -> Option<BindingInfo> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn require_compatible(
        &self,
        expected: Type,
        actual: Type,
        context: &str,
    ) -> Result<(), Diagnostic> {
        if compatible(expected, actual) {
            Ok(())
        } else {
            Err(
                Diagnostic::type_error(
                    "E0201",
                    format!("{context} expected {expected}, found {actual}"),
                )
                .with_label(format!("expected {expected}, found {actual}"))
                .with_help(format!("provide a value compatible with {expected}")),
            )
        }
    }

    fn require_exact(
        &self,
        expected: Type,
        actual: Type,
        context: &str,
    ) -> Result<(), Diagnostic> {
        if expected == actual {
            Ok(())
        } else {
            Err(
                Diagnostic::type_error(
                    "E0201",
                    format!("{context} expected {expected}, found {actual}"),
                )
                .with_label(format!("expected {expected}, found {actual}")),
            )
        }
    }
}

fn contains_try(expr: &Expr) -> bool {
    match expr {
        Expr::Try(_) => true,
        Expr::Some(value)
        | Expr::Ok(value)
        | Expr::Err(value)
        | Expr::Unary { expr: value, .. } => contains_try(value),
        Expr::Call { arguments, .. } => arguments.iter().any(contains_try),
        Expr::Binary { left, right, .. } => contains_try(left) || contains_try(right),
        Expr::Integer(_)
        | Expr::Float(_)
        | Expr::Bool(_)
        | Expr::String(_)
        | Expr::Variable(_)
        | Expr::None => false,
    }
}

fn compatible(expected: Type, actual: Type) -> bool {
    expected == actual || (expected == Type::Float && actual == Type::Int)
}

fn is_numeric(ty: Type) -> bool {
    matches!(ty, Type::Int | Type::Float)
}

fn is_plain_value(ty: Type) -> bool {
    matches!(ty, Type::Int | Type::Float | Type::Bool | Type::String)
}

fn numeric_result(left: Type, right: Type, operator: &str) -> Result<Type, Diagnostic> {
    if !is_numeric(left) || !is_numeric(right) {
        return Err(
            Diagnostic::type_error(
                "E0201",
                format!(
                    "operator '{operator}' requires numeric operands, found {left} and {right}"
                ),
            )
            .with_label("non-numeric operand"),
        );
    }
    Ok(if left == Type::Float || right == Type::Float {
        Type::Float
    } else {
        Type::Int
    })
}

fn block_definitely_returns(body: &[Stmt]) -> bool {
    body.iter().any(statement_definitely_returns)
}

fn statement_definitely_returns(statement: &Stmt) -> bool {
    match &statement.kind {
        StmtKind::Return(_) => true,
        StmtKind::Block(body) => block_definitely_returns(body),
        StmtKind::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => block_definitely_returns(then_branch) && block_definitely_returns(else_branch),
        StmtKind::Match { arms, .. } => {
            !arms.is_empty() && arms.iter().all(|arm| block_definitely_returns(&arm.body))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer::lex, parser::parse};

    fn check_source(source: &str) -> Result<(), Diagnostic> {
        check(&parse(lex(source)?)?)
    }

    #[test]
    fn accepts_option_and_exhaustive_match() {
        let source = "fn find() -> Option<string> { return Some(\"Genix\"); } fn main() { let x: Option<string> = find(); match x { Some(v) => { print(v); } None => { print(\"missing\"); } } }";
        assert!(check_source(source).is_ok());
    }

    #[test]
    fn accepts_result_and_try_propagation() {
        let source = "fn load() -> Result<string,string> { return Ok(\"data\"); } fn wrapper() -> Result<string,string> { let text: string = load()?; return Ok(text); } fn main() { let value: Result<string,string> = wrapper(); match value { Ok(v) => { print(v); } Err(e) => { print(e); } } }";
        assert!(check_source(source).is_ok());
    }

    #[test]
    fn rejects_non_exhaustive_match_with_code_and_span() {
        let source = "fn find() -> Option<string> { return None; } fn main() { let x: Option<string> = find(); match x { Some(v) => { print(v); } } }";
        let error = check_source(source).unwrap_err();
        assert_eq!(error.code, "E0205");
        assert!(error.message.contains("Option match"));
        assert!(error.span.is_some());
    }

    #[test]
    fn rejects_try_outside_result_function() {
        let source = "fn load() -> Result<string,string> { return Ok(\"data\"); } fn main() { let text: string = load()?; print(text); }";
        let error = check_source(source).unwrap_err();
        assert_eq!(error.code, "E0206");
        assert!(error.message.contains("function returning Result"));
    }

    #[test]
    fn reports_type_mismatch_code() {
        let source = "fn main() { let age: int = \"twenty\"; }";
        let error = check_source(source).unwrap_err();
        assert_eq!(error.code, "E0201");
        assert!(error.span.is_some());
    }
}
