use std::collections::HashMap;

use crate::ast::{BinaryOp, Expr, Function, Program, Stmt, Type, UnaryOp};

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

pub fn check(program: &Program) -> Result<(), String> {
    let signatures = collect_signatures(program)?;
    validate_main(&signatures)?;

    for function in &program.functions {
        let mut checker = Checker::new(&signatures, function.return_type);
        checker.check_function(function)?;

        if function.return_type != Type::Void && !block_definitely_returns(&function.body) {
            return Err(format!(
                "type error in function '{}': expected a guaranteed return value of type {}",
                function.name, function.return_type
            ));
        }
    }

    Ok(())
}

fn collect_signatures(program: &Program) -> Result<HashMap<String, Signature>, String> {
    let mut signatures = HashMap::new();

    for function in &program.functions {
        if signatures.contains_key(&function.name) {
            return Err(format!("type error: function '{}' is defined more than once", function.name));
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

fn validate_main(signatures: &HashMap<String, Signature>) -> Result<(), String> {
    let Some(main) = signatures.get("main") else {
        return Err("type error: Genix program must define fn main()".into());
    };

    if !main.params.is_empty() {
        return Err("type error: fn main() cannot take parameters in Genix v0.1".into());
    }
    if main.return_type != Type::Void {
        return Err("type error: fn main() cannot declare a return value in Genix v0.1".into());
    }

    Ok(())
}

struct Checker<'a> {
    signatures: &'a HashMap<String, Signature>,
    scopes: Vec<HashMap<String, BindingInfo>>,
    return_type: Type,
}

impl<'a> Checker<'a> {
    fn new(signatures: &'a HashMap<String, Signature>, return_type: Type) -> Self {
        Self {
            signatures,
            scopes: vec![HashMap::new()],
            return_type,
        }
    }

    fn check_function(&mut self, function: &Function) -> Result<(), String> {
        for param in &function.params {
            if param.ty == Type::Void {
                return Err(format!(
                    "type error in function '{}': parameter '{}' cannot have type void",
                    function.name, param.name
                ));
            }

            let scope = self.scopes.last_mut().expect("checker always has a scope");
            if scope.contains_key(&param.name) {
                return Err(format!(
                    "type error in function '{}': duplicate parameter '{}'",
                    function.name, param.name
                ));
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

    fn check_statement(&mut self, statement: &Stmt) -> Result<(), String> {
        match statement {
            Stmt::Let {
                name,
                value,
                mutable,
                annotation,
            } => {
                let actual = self.expression_type(value)?;
                if actual == Type::Void {
                    return Err(format!("type error: variable '{name}' cannot store a void value"));
                }

                let ty = if let Some(expected) = annotation {
                    if *expected == Type::Void {
                        return Err(format!("type error: variable '{name}' cannot have type void"));
                    }
                    self.require_compatible(*expected, actual, &format!("initializer for '{name}'"))?;
                    *expected
                } else {
                    actual
                };

                let scope = self.scopes.last_mut().expect("checker always has a scope");
                if scope.contains_key(name) {
                    return Err(format!("type error: variable '{name}' is already declared in this scope"));
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
            Stmt::Assign { name, value } => {
                let binding = self
                    .lookup(name)
                    .ok_or_else(|| format!("type error: undefined variable '{name}'"))?;
                if !binding.mutable {
                    return Err(format!(
                        "type error: cannot assign to immutable variable '{name}'; declare it with 'mut'"
                    ));
                }
                let actual = self.expression_type(value)?;
                self.require_compatible(binding.ty, actual, &format!("assignment to '{name}'"))
            }
            Stmt::Print(expr) => {
                let ty = self.expression_type(expr)?;
                if ty == Type::Void {
                    Err("type error: print() cannot print a void value".into())
                } else {
                    Ok(())
                }
            }
            Stmt::Expr(expr) => {
                if !matches!(expr, Expr::Call { .. }) {
                    return Err("type error: only function calls can be expression statements".into());
                }
                self.expression_type(expr)?;
                Ok(())
            }
            Stmt::Return(value) => match (self.return_type, value) {
                (Type::Void, None) => Ok(()),
                (Type::Void, Some(_)) => Err("type error: void function cannot return a value".into()),
                (expected, None) => Err(format!("type error: expected return value of type {expected}")),
                (expected, Some(expr)) => {
                    let actual = self.expression_type(expr)?;
                    self.require_compatible(expected, actual, "return value")
                }
            },
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.require_exact(Type::Bool, self.expression_type(condition)?, "if condition")?;
                self.check_block(then_branch)?;
                if let Some(else_branch) = else_branch {
                    self.check_block(else_branch)?;
                }
                Ok(())
            }
            Stmt::While { condition, body } => {
                self.require_exact(Type::Bool, self.expression_type(condition)?, "while condition")?;
                self.check_block(body)
            }
            Stmt::Block(body) => self.check_block(body),
        }
    }

    fn check_block(&mut self, body: &[Stmt]) -> Result<(), String> {
        self.scopes.push(HashMap::new());
        let result = (|| {
            for statement in body {
                self.check_statement(statement)?;
            }
            Ok(())
        })();
        self.scopes.pop();
        result
    }

    fn expression_type(&self, expr: &Expr) -> Result<Type, String> {
        match expr {
            Expr::Integer(_) => Ok(Type::Int),
            Expr::Float(_) => Ok(Type::Float),
            Expr::Bool(_) => Ok(Type::Bool),
            Expr::String(_) => Ok(Type::String),
            Expr::Variable(name) => self
                .lookup(name)
                .map(|binding| binding.ty)
                .ok_or_else(|| format!("type error: undefined variable '{name}'")),
            Expr::Call { callee, arguments } => {
                let signature = self
                    .signatures
                    .get(callee)
                    .cloned()
                    .ok_or_else(|| format!("type error: undefined function '{callee}'"))?;

                if arguments.len() != signature.params.len() {
                    return Err(format!(
                        "type error: function '{callee}' expects {} argument(s), found {}",
                        signature.params.len(),
                        arguments.len()
                    ));
                }

                for (index, (argument, expected)) in arguments.iter().zip(signature.params.iter()).enumerate() {
                    let actual = self.expression_type(argument)?;
                    self.require_compatible(
                        *expected,
                        actual,
                        &format!("argument {} for function '{callee}'", index + 1),
                    )?;
                }

                Ok(signature.return_type)
            }
            Expr::Unary { op, expr } => {
                let ty = self.expression_type(expr)?;
                match op {
                    UnaryOp::Negate if is_numeric(ty) => Ok(ty),
                    UnaryOp::Negate => Err(format!("type error: unary '-' requires a number, found {ty}")),
                    UnaryOp::Not if ty == Type::Bool => Ok(Type::Bool),
                    UnaryOp::Not => Err(format!("type error: operator '!' requires bool, found {ty}")),
                }
            }
            Expr::Binary { left, op, right } => {
                let left = self.expression_type(left)?;
                let right = self.expression_type(right)?;
                self.binary_type(left, *op, right)
            }
        }
    }

    fn binary_type(&self, left: Type, op: BinaryOp, right: Type) -> Result<Type, String> {
        match op {
            BinaryOp::Add => {
                if left == Type::String && right == Type::String {
                    Ok(Type::String)
                } else {
                    numeric_result(left, right, "+")
                }
            }
            BinaryOp::Subtract => numeric_result(left, right, "-"),
            BinaryOp::Multiply => numeric_result(left, right, "*"),
            BinaryOp::Divide => numeric_result(left, right, "/"),
            BinaryOp::Equal | BinaryOp::NotEqual => {
                if left == right || (is_numeric(left) && is_numeric(right)) {
                    Ok(Type::Bool)
                } else {
                    Err(format!(
                        "type error: equality comparison requires compatible types, found {left} and {right}"
                    ))
                }
            }
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                if is_numeric(left) && is_numeric(right) {
                    Ok(Type::Bool)
                } else {
                    Err(format!("type error: comparison requires numbers, found {left} and {right}"))
                }
            }
            BinaryOp::And | BinaryOp::Or => {
                if left == Type::Bool && right == Type::Bool {
                    Ok(Type::Bool)
                } else {
                    Err(format!("type error: logical operator requires bool operands, found {left} and {right}"))
                }
            }
        }
    }

    fn lookup(&self, name: &str) -> Option<BindingInfo> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn require_compatible(&self, expected: Type, actual: Type, context: &str) -> Result<(), String> {
        if compatible(expected, actual) {
            Ok(())
        } else {
            Err(format!("type error: {context} expected {expected}, found {actual}"))
        }
    }

    fn require_exact(&self, expected: Type, actual: Type, context: &str) -> Result<(), String> {
        if expected == actual {
            Ok(())
        } else {
            Err(format!("type error: {context} expected {expected}, found {actual}"))
        }
    }
}

fn compatible(expected: Type, actual: Type) -> bool {
    expected == actual || (expected == Type::Float && actual == Type::Int)
}

fn is_numeric(ty: Type) -> bool {
    matches!(ty, Type::Int | Type::Float)
}

fn numeric_result(left: Type, right: Type, operator: &str) -> Result<Type, String> {
    if !is_numeric(left) || !is_numeric(right) {
        return Err(format!(
            "type error: operator '{operator}' requires numeric operands, found {left} and {right}"
        ));
    }
    if left == Type::Float || right == Type::Float {
        Ok(Type::Float)
    } else {
        Ok(Type::Int)
    }
}

fn block_definitely_returns(body: &[Stmt]) -> bool {
    for statement in body {
        if statement_definitely_returns(statement) {
            return true;
        }
    }
    false
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
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer::lex, parser::parse};

    fn check_source(source: &str) -> Result<(), String> {
        check(&parse(lex(source)?)?)
    }

    #[test]
    fn accepts_typed_functions_and_calls() {
        let source = "fn add(a: int, b: int) -> int { return a + b; } fn main() { let result: int = add(2, 3); print(result); }";
        assert!(check_source(source).is_ok());
    }

    #[test]
    fn allows_int_to_float_widening() {
        let source = "fn scale(x: float) -> float { return x * 2.0; } fn main() { let result: float = scale(3); print(result); }";
        assert!(check_source(source).is_ok());
    }

    #[test]
    fn rejects_wrong_variable_type() {
        let source = "fn main() { let age: int = \"twenty\"; }";
        let error = check_source(source).unwrap_err();
        assert!(error.contains("expected int, found string"));
    }

    #[test]
    fn rejects_wrong_argument_type() {
        let source = "fn add(a: int, b: int) -> int { return a + b; } fn main() { print(add(1, \"two\")); }";
        let error = check_source(source).unwrap_err();
        assert!(error.contains("argument 2"));
    }

    #[test]
    fn requires_guaranteed_return() {
        let source = "fn positive(x: int) -> int { if x > 0 { return x; } } fn main() {}";
        let error = check_source(source).unwrap_err();
        assert!(error.contains("guaranteed return"));
    }
}
