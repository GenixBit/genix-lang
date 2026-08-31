use std::collections::HashMap;

use crate::ast::{BinaryOp, Expr, Function, MatchArm, Pattern, Program, Stmt, Type, UnaryOp};
use crate::diagnostics::{Diagnostic, Span};
use crate::source_map::{locate_text, SourceMap};

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

#[derive(Debug, Clone)]
enum SemanticLocator {
    First,
    Function(String),
    Name(String),
    Initializer(String),
    Assignment(String),
    Call(String),
    Return,
    If,
    While,
    Match,
}

#[derive(Debug, Clone)]
struct SemanticError {
    code: &'static str,
    message: String,
    label: Option<&'static str>,
    help: Option<&'static str>,
    function: Option<String>,
    locator: SemanticLocator,
    related_function: Option<String>,
}

impl SemanticError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        let (label, help) = semantic_metadata(code);
        Self {
            code,
            message: message.into(),
            label,
            help,
            function: None,
            locator: SemanticLocator::First,
            related_function: None,
        }
    }

    fn in_function(mut self, function: impl Into<String>) -> Self {
        self.function = Some(function.into());
        self
    }

    fn at(mut self, locator: SemanticLocator) -> Self {
        self.locator = locator;
        self
    }

    fn with_related_function(mut self, function: impl Into<String>) -> Self {
        self.related_function = Some(function.into());
        self
    }

    fn legacy(&self) -> String {
        if let Some(function) = &self.function {
            format!("type error in function '{}': {}", function, self.message)
        } else {
            format!("type error: {}", self.message)
        }
    }

    fn into_diagnostic(self, source_map: &SourceMap) -> Diagnostic {
        let mut diagnostic = Diagnostic::type_error(self.code, self.message.clone());
        if let Some(label) = self.label {
            diagnostic = diagnostic.with_label(label);
        }
        if let Some(help) = self.help {
            diagnostic = diagnostic.with_help(help);
        }

        let primary_file = self
            .function
            .as_deref()
            .and_then(|function| source_map.file_for_function(function))
            .or_else(|| source_map.entry());

        if let Some(file) = primary_file {
            diagnostic = diagnostic.with_source_name(file.name.clone());
            diagnostic.span = locate_semantic_span(&file.source, &self.locator)
                .or_else(|| first_code_span(&file.source));
        }

        if let Some(function) = self.function.as_deref() {
            if let Some((module, _)) = function.split_once('.') {
                if let Some((file, span)) = source_map.locate_module_reference(module) {
                    if diagnostic.source_name.as_deref() != Some(file.as_str()) {
                        diagnostic = diagnostic.with_related(file, span, "module referenced here");
                    }
                }
            }
        }

        if let Some(callee) = self.related_function.as_deref() {
            if let Some((file, span)) = source_map.locate_function(callee) {
                diagnostic = diagnostic.with_related(file, span, "function defined here");
            } else if let Some((module, _)) = callee.split_once('.') {
                if let Some((file, span)) = source_map.locate_module_reference(module) {
                    diagnostic = diagnostic.with_related(file, span, "module referenced here");
                }
            }
        }

        diagnostic
    }
}

fn semantic_metadata(code: &str) -> (Option<&'static str>, Option<&'static str>) {
    match code {
        "E0201" => (
            Some("type mismatch"),
            Some("change the expression or annotation so the types are compatible"),
        ),
        "E0202" => (
            Some("name is not defined here"),
            Some("check the spelling, declaration, or imported module"),
        ),
        "E0203" => (
            Some("immutable binding"),
            Some("declare the variable with `mut` if reassignment is intended"),
        ),
        "E0204" => (
            Some("invalid return"),
            Some("make every return value match the function return type"),
        ),
        "E0205" => (
            Some("invalid or non-exhaustive match"),
            Some("Option needs Some/None; Result needs Ok/Err"),
        ),
        "E0206" => (
            Some("invalid '?' propagation"),
            Some("use '?' only with Result<T,string> in a Result-returning function"),
        ),
        "E0207" => (
            Some("function call does not match its signature"),
            Some("check the number and types of the arguments"),
        ),
        "E0209" => (
            Some("duplicate declaration"),
            Some("rename or remove the duplicate declaration"),
        ),
        "E0210" => (
            None,
            Some("define `fn main() { ... }` with no parameters or return type"),
        ),
        _ => (None, None),
    }
}

pub fn check(program: &Program) -> Result<(), String> {
    check_semantic(program).map_err(|error| error.legacy())
}

pub fn check_diagnostic(program: &Program, source_map: &SourceMap) -> Result<(), Diagnostic> {
    check_semantic(program).map_err(|error| error.into_diagnostic(source_map))
}

fn check_semantic(program: &Program) -> Result<(), SemanticError> {
    let signatures = collect_signatures(program)?;
    validate_main(&signatures)?;
    for function in &program.functions {
        let mut checker = Checker::new(&signatures, function);
        checker.check_function(function)?;
        if function.return_type != Type::Void && !block_definitely_returns(&function.body) {
            return Err(
                SemanticError::new(
                    "E0204",
                    format!("expected a guaranteed return value of type {}", function.return_type),
                )
                .in_function(function.name.clone())
                .at(SemanticLocator::Function(function.name.clone())),
            );
        }
    }
    Ok(())
}

fn collect_signatures(program: &Program) -> Result<HashMap<String, Signature>, SemanticError> {
    let mut signatures = HashMap::new();
    for function in &program.functions {
        if signatures.contains_key(&function.name) {
            return Err(
                SemanticError::new(
                    "E0209",
                    format!("function '{}' is defined more than once", function.name),
                )
                .in_function(function.name.clone())
                .at(SemanticLocator::Function(function.name.clone())),
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

fn validate_main(signatures: &HashMap<String, Signature>) -> Result<(), SemanticError> {
    let Some(main) = signatures.get("main") else {
        return Err(SemanticError::new(
            "E0210",
            "Genix program must define fn main()",
        ));
    };
    if !main.params.is_empty() {
        return Err(
            SemanticError::new("E0210", "fn main() cannot take parameters in Genix v0.1")
                .in_function("main")
                .at(SemanticLocator::Function("main".into())),
        );
    }
    if main.return_type != Type::Void {
        return Err(
            SemanticError::new(
                "E0210",
                "fn main() cannot declare a return value in Genix v0.1",
            )
            .in_function("main")
            .at(SemanticLocator::Function("main".into())),
        );
    }
    Ok(())
}

struct Checker<'a> {
    signatures: &'a HashMap<String, Signature>,
    scopes: Vec<HashMap<String, BindingInfo>>,
    return_type: Type,
    function_name: String,
}

impl<'a> Checker<'a> {
    fn new(signatures: &'a HashMap<String, Signature>, function: &Function) -> Self {
        Self {
            signatures,
            scopes: vec![HashMap::new()],
            return_type: function.return_type,
            function_name: function.name.clone(),
        }
    }

    fn error(
        &self,
        code: &'static str,
        message: impl Into<String>,
        locator: SemanticLocator,
    ) -> SemanticError {
        SemanticError::new(code, message)
            .in_function(self.function_name.clone())
            .at(locator)
    }

    fn check_function(&mut self, function: &Function) -> Result<(), SemanticError> {
        for param in &function.params {
            if param.ty == Type::Void {
                return Err(self.error(
                    "E0208",
                    format!("parameter '{}' cannot have type void", param.name),
                    SemanticLocator::Name(param.name.clone()),
                ));
            }
            if self.scopes.last().unwrap().contains_key(&param.name) {
                return Err(self.error(
                    "E0209",
                    format!("duplicate parameter '{}'", param.name),
                    SemanticLocator::Name(param.name.clone()),
                ));
            }
            self.scopes.last_mut().unwrap().insert(
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

    fn check_statement(&mut self, statement: &Stmt) -> Result<(), SemanticError> {
        match statement {
            Stmt::Let {
                name,
                value,
                mutable,
                annotation,
            } => {
                self.ensure_try_position(value, true, "variable initializer")?;
                let actual = self.expression_type_expected(value, *annotation)?;
                if actual == Type::Void {
                    return Err(self.error(
                        "E0208",
                        format!("variable '{name}' cannot store a void value"),
                        SemanticLocator::Initializer(name.clone()),
                    ));
                }
                let ty = if let Some(expected) = annotation {
                    if *expected == Type::Void {
                        return Err(self.error(
                            "E0208",
                            format!("variable '{name}' cannot have type void"),
                            SemanticLocator::Name(name.clone()),
                        ));
                    }
                    self.require_compatible(
                        *expected,
                        actual,
                        &format!("initializer for '{name}'"),
                        SemanticLocator::Initializer(name.clone()),
                    )?;
                    *expected
                } else {
                    actual
                };
                if self.scopes.last().unwrap().contains_key(name) {
                    return Err(self.error(
                        "E0209",
                        format!("variable '{name}' is already declared in this scope"),
                        SemanticLocator::Name(name.clone()),
                    ));
                }
                self.scopes.last_mut().unwrap().insert(
                    name.clone(),
                    BindingInfo {
                        ty,
                        mutable: *mutable,
                    },
                );
                Ok(())
            }
            Stmt::Assign { name, value } => {
                self.ensure_try_position(value, true, "assignment")?;
                let binding = self.lookup(name).ok_or_else(|| {
                    self.error(
                        "E0202",
                        format!("undefined variable '{name}'"),
                        SemanticLocator::Name(name.clone()),
                    )
                })?;
                if !binding.mutable {
                    return Err(self.error(
                        "E0203",
                        format!(
                            "cannot assign to immutable variable '{name}'; declare it with 'mut'"
                        ),
                        SemanticLocator::Assignment(name.clone()),
                    ));
                }
                let actual = self.expression_type_expected(value, Some(binding.ty))?;
                self.require_compatible(
                    binding.ty,
                    actual,
                    &format!("assignment to '{name}'"),
                    SemanticLocator::Assignment(name.clone()),
                )
            }
            Stmt::Print(expr) => {
                self.ensure_try_position(expr, false, "print argument")?;
                let ty = self.expression_type(expr)?;
                if ty == Type::Void {
                    return Err(self.error(
                        "E0208",
                        "print() cannot print a void value",
                        SemanticLocator::Call("print".into()),
                    ));
                }
                if ty.option_inner().is_some() || ty.result_ok().is_some() {
                    return Err(self.error(
                        "E0208",
                        format!("print() cannot directly print {ty}; use match to unwrap it"),
                        SemanticLocator::Call("print".into()),
                    ));
                }
                Ok(())
            }
            Stmt::Expr(expr) => {
                self.ensure_try_position(expr, true, "expression statement")?;
                if !matches!(expr, Expr::Call { .. } | Expr::Try(_)) {
                    return Err(self.error(
                        "E0208",
                        "only function calls can be expression statements",
                        SemanticLocator::First,
                    ));
                }
                self.expression_type(expr)?;
                Ok(())
            }
            Stmt::Return(value) => match (self.return_type, value) {
                (Type::Void, None) => Ok(()),
                (Type::Void, Some(_)) => Err(self.error(
                    "E0204",
                    "void function cannot return a value",
                    SemanticLocator::Return,
                )),
                (expected, None) => Err(self.error(
                    "E0204",
                    format!("expected return value of type {expected}"),
                    SemanticLocator::Return,
                )),
                (expected, Some(expr)) => {
                    self.ensure_try_position(expr, false, "return value")?;
                    let actual = self.expression_type_expected(expr, Some(expected))?;
                    self.require_compatible(
                        expected,
                        actual,
                        "return value",
                        SemanticLocator::Return,
                    )
                }
            },
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.ensure_try_position(condition, false, "if condition")?;
                let actual = self.expression_type(condition)?;
                self.require_exact(
                    Type::Bool,
                    actual,
                    "if condition",
                    SemanticLocator::If,
                )?;
                self.check_block(then_branch)?;
                if let Some(branch) = else_branch {
                    self.check_block(branch)?;
                }
                Ok(())
            }
            Stmt::While { condition, body } => {
                self.ensure_try_position(condition, false, "while condition")?;
                let actual = self.expression_type(condition)?;
                self.require_exact(
                    Type::Bool,
                    actual,
                    "while condition",
                    SemanticLocator::While,
                )?;
                self.check_block(body)
            }
            Stmt::Match { value, arms } => self.check_match(value, arms),
            Stmt::Block(body) => self.check_block(body),
        }
    }

    fn check_match(&mut self, value: &Expr, arms: &[MatchArm]) -> Result<(), SemanticError> {
        self.ensure_try_position(value, false, "match value")?;
        let ty = self.expression_type(value)?;
        let is_option = ty.option_inner().is_some();
        let is_result = ty.result_ok().is_some();
        if !is_option && !is_result {
            return Err(self.error(
                "E0205",
                format!("match currently supports Option and Result, found {ty}"),
                SemanticLocator::Match,
            ));
        }

        let mut first_seen = false;
        let mut second_seen = false;
        for arm in arms {
            let binding = match (&arm.pattern, ty) {
                (Pattern::Some(name), option) if option.option_inner().is_some() => {
                    if first_seen {
                        return Err(self.error(
                            "E0205",
                            "duplicate Some match arm",
                            SemanticLocator::Match,
                        ));
                    }
                    first_seen = true;
                    Some((name.as_str(), option.option_inner().unwrap()))
                }
                (Pattern::None, option) if option.option_inner().is_some() => {
                    if second_seen {
                        return Err(self.error(
                            "E0205",
                            "duplicate None match arm",
                            SemanticLocator::Match,
                        ));
                    }
                    second_seen = true;
                    None
                }
                (Pattern::Ok(name), result) if result.result_ok().is_some() => {
                    if first_seen {
                        return Err(self.error(
                            "E0205",
                            "duplicate Ok match arm",
                            SemanticLocator::Match,
                        ));
                    }
                    first_seen = true;
                    Some((name.as_str(), result.result_ok().unwrap()))
                }
                (Pattern::Err(name), result) if result.result_ok().is_some() => {
                    if second_seen {
                        return Err(self.error(
                            "E0205",
                            "duplicate Err match arm",
                            SemanticLocator::Match,
                        ));
                    }
                    second_seen = true;
                    Some((name.as_str(), Type::String))
                }
                (pattern, _) => {
                    return Err(self.error(
                        "E0205",
                        format!("pattern {pattern:?} is not valid for {ty}"),
                        SemanticLocator::Match,
                    ));
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
            let result: Result<(), SemanticError> = (|| {
                for statement in &arm.body {
                    self.check_statement(statement)?;
                }
                Ok(())
            })();
            self.scopes.pop();
            result?;
        }

        if is_option && !(first_seen && second_seen) {
            return Err(self.error(
                "E0205",
                "Option match must handle both Some(...) and None",
                SemanticLocator::Match,
            ));
        }
        if is_result && !(first_seen && second_seen) {
            return Err(self.error(
                "E0205",
                "Result match must handle both Ok(...) and Err(...)",
                SemanticLocator::Match,
            ));
        }
        Ok(())
    }

    fn check_block(&mut self, body: &[Stmt]) -> Result<(), SemanticError> {
        self.scopes.push(HashMap::new());
        let result: Result<(), SemanticError> = (|| {
            for statement in body {
                self.check_statement(statement)?;
            }
            Ok(())
        })();
        self.scopes.pop();
        result
    }

    fn expression_type(&self, expr: &Expr) -> Result<Type, SemanticError> {
        self.expression_type_expected(expr, None)
    }

    fn expression_type_expected(
        &self,
        expr: &Expr,
        expected: Option<Type>,
    ) -> Result<Type, SemanticError> {
        match expr {
            Expr::Integer(_) => Ok(Type::Int),
            Expr::Float(_) => Ok(Type::Float),
            Expr::Bool(_) => Ok(Type::Bool),
            Expr::String(_) => Ok(Type::String),
            Expr::Variable(name) => self.lookup(name).map(|binding| binding.ty).ok_or_else(|| {
                self.error(
                    "E0202",
                    format!("undefined variable '{name}'"),
                    SemanticLocator::Name(name.clone()),
                )
            }),
            Expr::Call { callee, arguments } => {
                let signature = self.signatures.get(callee).cloned().ok_or_else(|| {
                    self.error(
                        "E0202",
                        format!("undefined function '{callee}'"),
                        SemanticLocator::Call(callee.clone()),
                    )
                    .with_related_function(callee.clone())
                })?;
                if arguments.len() != signature.params.len() {
                    return Err(
                        self.error(
                            "E0207",
                            format!(
                                "function '{callee}' expects {} argument(s), found {}",
                                signature.params.len(),
                                arguments.len()
                            ),
                            SemanticLocator::Call(callee.clone()),
                        )
                        .with_related_function(callee.clone()),
                    );
                }
                for (index, (argument, param_ty)) in
                    arguments.iter().zip(signature.params.iter()).enumerate()
                {
                    let actual = self.expression_type_expected(argument, Some(*param_ty))?;
                    if !compatible(*param_ty, actual) {
                        return Err(
                            self.error(
                                "E0207",
                                format!(
                                    "argument {} for function '{callee}' expected {param_ty}, found {actual}",
                                    index + 1
                                ),
                                SemanticLocator::Call(callee.clone()),
                            )
                            .with_related_function(callee.clone()),
                        );
                    }
                }
                Ok(signature.return_type)
            }
            Expr::Some(value) => {
                if let Some(option_ty) = expected.filter(|ty| ty.option_inner().is_some()) {
                    let inner = option_ty.option_inner().unwrap();
                    let actual = self.expression_type_expected(value, Some(inner))?;
                    self.require_compatible(
                        inner,
                        actual,
                        "Some value",
                        SemanticLocator::Name("Some".into()),
                    )?;
                    Ok(option_ty)
                } else {
                    let inner = self.expression_type(value)?;
                    Type::option(inner).ok_or_else(|| {
                        self.error(
                            "E0208",
                            format!("Some({inner}) is not supported in Genix v0.1"),
                            SemanticLocator::Name("Some".into()),
                        )
                    })
                }
            }
            Expr::None => expected
                .filter(|ty| ty.option_inner().is_some())
                .ok_or_else(|| {
                    self.error(
                        "E0208",
                        "None requires an Option<T> type annotation or return context",
                        SemanticLocator::Name("None".into()),
                    )
                }),
            Expr::Ok(value) => {
                if let Some(result_ty) = expected.filter(|ty| ty.result_ok().is_some()) {
                    let ok_ty = result_ty.result_ok().unwrap();
                    let actual = self.expression_type_expected(value, Some(ok_ty))?;
                    self.require_compatible(
                        ok_ty,
                        actual,
                        "Ok value",
                        SemanticLocator::Name("Ok".into()),
                    )?;
                    Ok(result_ty)
                } else {
                    let ok_ty = self.expression_type(value)?;
                    Type::result(ok_ty, Type::String).ok_or_else(|| {
                        self.error(
                            "E0208",
                            format!("Ok({ok_ty}) is not supported in Genix v0.1"),
                            SemanticLocator::Name("Ok".into()),
                        )
                    })
                }
            }
            Expr::Err(error) => {
                let error_ty = self.expression_type_expected(error, Some(Type::String))?;
                self.require_exact(
                    Type::String,
                    error_ty,
                    "Err value",
                    SemanticLocator::Name("Err".into()),
                )?;
                expected
                    .filter(|ty| ty.result_ok().is_some())
                    .ok_or_else(|| {
                        self.error(
                            "E0208",
                            "Err(...) requires a Result<T,string> type annotation or return context",
                            SemanticLocator::Name("Err".into()),
                        )
                    })
            }
            Expr::Try(inner) => {
                let result_ty = self.expression_type(inner)?;
                let Some(ok_ty) = result_ty.result_ok() else {
                    return Err(self.error(
                        "E0206",
                        format!("'?' requires Result<T,string>, found {result_ty}"),
                        SemanticLocator::Name("?".into()),
                    ));
                };
                if !self.return_type.is_result() {
                    return Err(self.error(
                        "E0206",
                        format!(
                            "'?' can only be used inside a function returning Result<T,string>; current return type is {}",
                            self.return_type
                        ),
                        SemanticLocator::Name("?".into()),
                    ));
                }
                Ok(ok_ty)
            }
            Expr::Unary { op, expr } => {
                let ty = self.expression_type(expr)?;
                match op {
                    UnaryOp::Negate if is_numeric(ty) => Ok(ty),
                    UnaryOp::Negate => Err(self.error(
                        "E0201",
                        format!("unary '-' requires a number, found {ty}"),
                        SemanticLocator::Name("-".into()),
                    )),
                    UnaryOp::Not if ty == Type::Bool => Ok(Type::Bool),
                    UnaryOp::Not => Err(self.error(
                        "E0201",
                        format!("operator '!' requires bool, found {ty}"),
                        SemanticLocator::Name("!".into()),
                    )),
                }
            }
            Expr::Binary { left, op, right } => {
                let left_ty = self.expression_type(left)?;
                let right_ty = self.expression_type(right)?;
                self.binary_type(left_ty, *op, right_ty)
            }
        }
    }

    fn binary_type(
        &self,
        left: Type,
        op: BinaryOp,
        right: Type,
    ) -> Result<Type, SemanticError> {
        match op {
            BinaryOp::Add if left == Type::String && right == Type::String => Ok(Type::String),
            BinaryOp::Add => self.numeric_result(left, right, "+"),
            BinaryOp::Subtract => self.numeric_result(left, right, "-"),
            BinaryOp::Multiply => self.numeric_result(left, right, "*"),
            BinaryOp::Divide => self.numeric_result(left, right, "/"),
            BinaryOp::Equal | BinaryOp::NotEqual => {
                if (left == right && is_plain_value(left))
                    || (is_numeric(left) && is_numeric(right))
                {
                    Ok(Type::Bool)
                } else {
                    Err(self.error(
                        "E0201",
                        format!(
                            "equality comparison requires compatible primitive types, found {left} and {right}"
                        ),
                        SemanticLocator::First,
                    ))
                }
            }
            BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual => {
                if is_numeric(left) && is_numeric(right) {
                    Ok(Type::Bool)
                } else {
                    Err(self.error(
                        "E0201",
                        format!("comparison requires numbers, found {left} and {right}"),
                        SemanticLocator::First,
                    ))
                }
            }
            BinaryOp::And | BinaryOp::Or => {
                if left == Type::Bool && right == Type::Bool {
                    Ok(Type::Bool)
                } else {
                    Err(self.error(
                        "E0201",
                        format!(
                            "logical operator requires bool operands, found {left} and {right}"
                        ),
                        SemanticLocator::First,
                    ))
                }
            }
        }
    }

    fn numeric_result(
        &self,
        left: Type,
        right: Type,
        operator: &str,
    ) -> Result<Type, SemanticError> {
        if !is_numeric(left) || !is_numeric(right) {
            return Err(self.error(
                "E0201",
                format!(
                    "operator '{operator}' requires numeric operands, found {left} and {right}"
                ),
                SemanticLocator::Name(operator.into()),
            ));
        }
        Ok(if left == Type::Float || right == Type::Float {
            Type::Float
        } else {
            Type::Int
        })
    }

    fn ensure_try_position(
        &self,
        expr: &Expr,
        allow_root: bool,
        context: &str,
    ) -> Result<(), SemanticError> {
        if contains_try(expr) && !(allow_root && matches!(expr, Expr::Try(_))) {
            return Err(self.error(
                "E0206",
                format!(
                    "'?' is currently supported only as the complete value of a variable initializer, assignment, or call statement; not inside {context}"
                ),
                SemanticLocator::Name("?".into()),
            ));
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
        locator: SemanticLocator,
    ) -> Result<(), SemanticError> {
        if compatible(expected, actual) {
            Ok(())
        } else {
            Err(self.error(
                "E0201",
                format!("{context} expected {expected}, found {actual}"),
                locator,
            ))
        }
    }

    fn require_exact(
        &self,
        expected: Type,
        actual: Type,
        context: &str,
        locator: SemanticLocator,
    ) -> Result<(), SemanticError> {
        if expected == actual {
            Ok(())
        } else {
            Err(self.error(
                "E0201",
                format!("{context} expected {expected}, found {actual}"),
                locator,
            ))
        }
    }
}

fn locate_semantic_span(source: &str, locator: &SemanticLocator) -> Option<Span> {
    match locator {
        SemanticLocator::First => first_code_span(source),
        SemanticLocator::Function(name) => {
            let local = name.rsplit('.').next().unwrap_or(name);
            locate_text(source, &format!("fn {local}"))
        }
        SemanticLocator::Name(name) => locate_text(source, name),
        SemanticLocator::Initializer(name) => initializer_span(source, name),
        SemanticLocator::Assignment(name) => assignment_span(source, name),
        SemanticLocator::Call(callee) => call_span(source, callee),
        SemanticLocator::Return => locate_text(source, "return"),
        SemanticLocator::If => locate_text(source, "if ").map(keyword_span),
        SemanticLocator::While => locate_text(source, "while ").map(keyword_span),
        SemanticLocator::Match => locate_text(source, "match ").map(keyword_span),
    }
}

fn keyword_span(span: Span) -> Span {
    Span::single(
        span.line,
        span.column,
        span.end_column.saturating_sub(span.column).max(1),
    )
}

fn initializer_span(source: &str, name: &str) -> Option<Span> {
    for (index, line) in source.lines().enumerate() {
        if !(line.contains(&format!("let {name}")) || line.contains(&format!("mut {name}"))) {
            continue;
        }
        let equal = line.find('=')?;
        let after_equal = &line[equal + 1..];
        let leading = after_equal.chars().take_while(|ch| ch.is_whitespace()).count();
        let value_start = equal + 1 + leading;
        let value_text = after_equal.trim_start().trim_end_matches(';').trim_end();
        return Some(Span::single(
            index + 1,
            value_start + 1,
            value_text.chars().count().max(1),
        ));
    }
    None
}

fn assignment_span(source: &str, name: &str) -> Option<Span> {
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(&format!("{name} =")) {
            let column = line.chars().count() - trimmed.chars().count() + 1;
            return Some(Span::single(index + 1, column, name.chars().count()));
        }
    }
    locate_text(source, name)
}

fn call_span(source: &str, callee: &str) -> Option<Span> {
    let local = callee.rsplit('.').next().unwrap_or(callee);
    for needle in [format!("{callee}("), format!("{local}(")] {
        for (index, line) in source.lines().enumerate() {
            if line.contains(&format!("fn {local}(")) {
                continue;
            }
            if let Some(byte_column) = line.find(&needle) {
                let column = line[..byte_column].chars().count() + 1;
                return Some(Span::single(
                    index + 1,
                    column,
                    needle.chars().count().saturating_sub(1).max(1),
                ));
            }
        }
    }
    None
}

fn first_code_span(source: &str) -> Option<Span> {
    source.lines().enumerate().find_map(|(index, line)| {
        if line.trim().is_empty() {
            return None;
        }
        let column = line.chars().position(|ch| !ch.is_whitespace()).unwrap_or(0) + 1;
        Some(Span::single(
            index + 1,
            column,
            line.trim().chars().count().max(1),
        ))
    })
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

fn block_definitely_returns(body: &[Stmt]) -> bool {
    body.iter().any(statement_definitely_returns)
}

fn statement_definitely_returns(statement: &Stmt) -> bool {
    match statement {
        Stmt::Return(_) => true,
        Stmt::Block(body) => block_definitely_returns(body),
        Stmt::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => block_definitely_returns(then_branch) && block_definitely_returns(else_branch),
        Stmt::Match { arms, .. } => {
            !arms.is_empty() && arms.iter().all(|arm| block_definitely_returns(&arm.body))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer::lex, parser::parse};

    fn parse_source(source: &str) -> Program {
        parse(lex(source).unwrap()).unwrap()
    }

    fn check_source(source: &str) -> Result<(), String> {
        check(&parse_source(source))
    }

    fn diagnostic_for(source: &str) -> Diagnostic {
        let program = parse_source(source);
        let mut source_map = SourceMap::new();
        source_map.add_file("test.gb", source);
        source_map.set_entry("test.gb");
        for function in &program.functions {
            source_map.bind_function(function.name.clone(), "test.gb");
        }
        check_diagnostic(&program, &source_map).unwrap_err()
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
    fn rejects_non_exhaustive_match() {
        let source = "fn find() -> Option<string> { return None; } fn main() { let x: Option<string> = find(); match x { Some(v) => { print(v); } } }";
        assert!(check_source(source).unwrap_err().contains("Option match"));
    }

    #[test]
    fn rejects_try_outside_result_function() {
        let source = "fn load() -> Result<string,string> { return Ok(\"data\"); } fn main() { let text: string = load()?; print(text); }";
        assert!(check_source(source)
            .unwrap_err()
            .contains("function returning Result"));
    }

    #[test]
    fn checker_owns_semantic_error_code_and_initializer_span() {
        let source = "fn main() {\n    let age: int = \"twenty\";\n}\n";
        let diagnostic = diagnostic_for(source);
        assert_eq!(diagnostic.code, "E0201");
        assert_eq!(diagnostic.source_name.as_deref(), Some("test.gb"));
        assert_eq!(diagnostic.span.unwrap().line, 2);
        assert!(diagnostic.span.unwrap().column > 10);
        assert_eq!(diagnostic.label.as_deref(), Some("type mismatch"));
    }

    #[test]
    fn checker_owns_call_signature_code() {
        let source = "fn add(a: int) -> int { return a; }\nfn main() { print(add(1, 2)); }\n";
        let diagnostic = diagnostic_for(source);
        assert_eq!(diagnostic.code, "E0207");
        assert!(diagnostic.message.contains("expects 1 argument"));
        assert!(!diagnostic.related.is_empty());
    }
}
