use std::collections::HashMap;
use std::fmt;

use crate::ast::{BinaryOp, Expr, Function, Program, Stmt, Type, UnaryOp};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Bool(bool),
    String(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(value) => write!(f, "{value}"),
            Value::Float(value) => write!(f, "{value}"),
            Value::Bool(value) => write!(f, "{value}"),
            Value::String(value) => write!(f, "{value}"),
        }
    }
}

#[derive(Debug, Clone)]
struct Binding {
    value: Value,
    mutable: bool,
    ty: Type,
}

#[derive(Debug)]
enum Flow {
    Continue,
    Return(Option<Value>),
}

pub fn execute(program: &Program) -> Result<(), String> {
    let mut interpreter = Interpreter::new(program);
    let result = interpreter.call_function("main", Vec::new())?;
    if result.is_some() {
        return Err("runtime error: fn main() cannot return a value".into());
    }
    Ok(())
}

struct Interpreter {
    functions: HashMap<String, Function>,
    scopes: Vec<HashMap<String, Binding>>,
}

impl Interpreter {
    fn new(program: &Program) -> Self {
        let functions = program
            .functions
            .iter()
            .cloned()
            .map(|function| (function.name.clone(), function))
            .collect();

        Self {
            functions,
            scopes: vec![HashMap::new()],
        }
    }

    fn call_function(&mut self, name: &str, arguments: Vec<Value>) -> Result<Option<Value>, String> {
        let function = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| format!("undefined function '{name}'"))?;

        if arguments.len() != function.params.len() {
            return Err(format!(
                "function '{name}' expects {} argument(s), found {}",
                function.params.len(),
                arguments.len()
            ));
        }

        let caller_scopes = std::mem::replace(&mut self.scopes, vec![HashMap::new()]);
        let result = (|| {
            for (param, value) in function.params.iter().zip(arguments.into_iter()) {
                let value = coerce(value, param.ty)?;
                self.scopes[0].insert(
                    param.name.clone(),
                    Binding {
                        value,
                        mutable: false,
                        ty: param.ty,
                    },
                );
            }

            for statement in &function.body {
                match self.execute_statement(statement)? {
                    Flow::Continue => {}
                    Flow::Return(value) => {
                        return self.finish_return(&function, value);
                    }
                }
            }

            if function.return_type == Type::Void {
                Ok(None)
            } else {
                Err(format!(
                    "function '{}' finished without returning {}",
                    function.name, function.return_type
                ))
            }
        })();
        self.scopes = caller_scopes;
        result
    }

    fn finish_return(&self, function: &Function, value: Option<Value>) -> Result<Option<Value>, String> {
        match (function.return_type, value) {
            (Type::Void, None) => Ok(None),
            (Type::Void, Some(_)) => Err(format!("void function '{}' returned a value", function.name)),
            (expected, Some(value)) => Ok(Some(coerce(value, expected)?)),
            (expected, None) => Err(format!(
                "function '{}' must return a value of type {expected}",
                function.name
            )),
        }
    }

    fn execute_statement(&mut self, statement: &Stmt) -> Result<Flow, String> {
        match statement {
            Stmt::Let {
                name,
                value,
                mutable,
                annotation,
            } => {
                let evaluated = self.evaluate(value)?;
                let ty = annotation.unwrap_or_else(|| value_type(&evaluated));
                let evaluated = coerce(evaluated, ty)?;
                let scope = self.scopes.last_mut().expect("interpreter always has a scope");
                if scope.contains_key(name) {
                    return Err(format!("variable '{name}' is already declared in this scope"));
                }
                scope.insert(
                    name.clone(),
                    Binding {
                        value: evaluated,
                        mutable: *mutable,
                        ty,
                    },
                );
                Ok(Flow::Continue)
            }
            Stmt::Assign { name, value } => {
                let evaluated = self.evaluate(value)?;
                self.assign(name, evaluated)?;
                Ok(Flow::Continue)
            }
            Stmt::Print(expr) => {
                println!("{}", self.evaluate(expr)?);
                Ok(Flow::Continue)
            }
            Stmt::Expr(expr) => {
                match expr {
                    Expr::Call { callee, arguments } => {
                        let values = self.evaluate_arguments(arguments)?;
                        self.call_function(callee, values)?;
                    }
                    _ => {
                        self.evaluate(expr)?;
                    }
                }
                Ok(Flow::Continue)
            }
            Stmt::Return(value) => {
                let value = match value {
                    Some(expr) => Some(self.evaluate(expr)?),
                    None => None,
                };
                Ok(Flow::Return(value))
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if self.expect_bool(self.evaluate(condition)?, "if condition")? {
                    self.execute_block(then_branch)
                } else if let Some(else_branch) = else_branch {
                    self.execute_block(else_branch)
                } else {
                    Ok(Flow::Continue)
                }
            }
            Stmt::While { condition, body } => {
                loop {
                    let condition_value = self.evaluate(condition)?;
                    if !self.expect_bool(condition_value, "while condition")? {
                        break;
                    }
                    match self.execute_block(body)? {
                        Flow::Continue => {}
                        returned @ Flow::Return(_) => return Ok(returned),
                    }
                }
                Ok(Flow::Continue)
            }
            Stmt::Block(body) => self.execute_block(body),
        }
    }

    fn execute_block(&mut self, body: &[Stmt]) -> Result<Flow, String> {
        self.scopes.push(HashMap::new());
        let result = (|| {
            for statement in body {
                match self.execute_statement(statement)? {
                    Flow::Continue => {}
                    returned @ Flow::Return(_) => return Ok(returned),
                }
            }
            Ok(Flow::Continue)
        })();
        self.scopes.pop();
        result
    }

    fn assign(&mut self, name: &str, value: Value) -> Result<(), String> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(binding) = scope.get_mut(name) {
                if !binding.mutable {
                    return Err(format!(
                        "cannot assign to immutable variable '{name}'; declare it with 'mut'"
                    ));
                }
                binding.value = coerce(value, binding.ty)?;
                return Ok(());
            }
        }
        Err(format!("undefined variable '{name}'"))
    }

    fn lookup(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn evaluate_arguments(&mut self, arguments: &[Expr]) -> Result<Vec<Value>, String> {
        let mut values = Vec::with_capacity(arguments.len());
        for argument in arguments {
            values.push(self.evaluate(argument)?);
        }
        Ok(values)
    }

    fn evaluate(&mut self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::Integer(value) => Ok(Value::Integer(*value)),
            Expr::Float(value) => Ok(Value::Float(*value)),
            Expr::Bool(value) => Ok(Value::Bool(*value)),
            Expr::String(value) => Ok(Value::String(value.clone())),
            Expr::Variable(name) => self
                .lookup(name)
                .map(|binding| binding.value.clone())
                .ok_or_else(|| format!("undefined variable '{name}'")),
            Expr::Call { callee, arguments } => {
                let values = self.evaluate_arguments(arguments)?;
                self.call_function(callee, values)?
                    .ok_or_else(|| format!("function '{callee}' does not return a value"))
            }
            Expr::Unary { op, expr } => {
                let value = self.evaluate(expr)?;
                self.apply_unary(*op, value)
            }
            Expr::Binary { left, op, right } => match op {
                BinaryOp::And => {
                    let left_value = self.evaluate(left)?;
                    let left = self.expect_bool(left_value, "left side of '&&'")?;
                    if !left {
                        return Ok(Value::Bool(false));
                    }
                    let right_value = self.evaluate(right)?;
                    let right = self.expect_bool(right_value, "right side of '&&'")?;
                    Ok(Value::Bool(right))
                }
                BinaryOp::Or => {
                    let left_value = self.evaluate(left)?;
                    let left = self.expect_bool(left_value, "left side of '||'")?;
                    if left {
                        return Ok(Value::Bool(true));
                    }
                    let right_value = self.evaluate(right)?;
                    let right = self.expect_bool(right_value, "right side of '||'")?;
                    Ok(Value::Bool(right))
                }
                _ => {
                    let left = self.evaluate(left)?;
                    let right = self.evaluate(right)?;
                    self.apply_binary(left, *op, right)
                }
            },
        }
    }

    fn expect_bool(&self, value: Value, context: &str) -> Result<bool, String> {
        match value {
            Value::Bool(value) => Ok(value),
            other => Err(format!("{context} must be bool, found {}", type_name(&other))),
        }
    }

    fn apply_unary(&self, op: UnaryOp, value: Value) -> Result<Value, String> {
        match (op, value) {
            (UnaryOp::Negate, Value::Integer(value)) => Ok(Value::Integer(-value)),
            (UnaryOp::Negate, Value::Float(value)) => Ok(Value::Float(-value)),
            (UnaryOp::Not, Value::Bool(value)) => Ok(Value::Bool(!value)),
            (UnaryOp::Negate, other) => Err(format!("cannot negate value of type {}", type_name(&other))),
            (UnaryOp::Not, other) => Err(format!("operator '!' requires bool, found {}", type_name(&other))),
        }
    }

    fn apply_binary(&self, left: Value, op: BinaryOp, right: Value) -> Result<Value, String> {
        match op {
            BinaryOp::Add => add(left, right),
            BinaryOp::Subtract => numeric_binary(left, right, |a, b| a - b, |a, b| a - b),
            BinaryOp::Multiply => numeric_binary(left, right, |a, b| a * b, |a, b| a * b),
            BinaryOp::Divide => divide(left, right),
            BinaryOp::Equal => Ok(Value::Bool(values_equal(&left, &right))),
            BinaryOp::NotEqual => Ok(Value::Bool(!values_equal(&left, &right))),
            BinaryOp::Less => compare_numeric(left, right, |a, b| a < b),
            BinaryOp::LessEqual => compare_numeric(left, right, |a, b| a <= b),
            BinaryOp::Greater => compare_numeric(left, right, |a, b| a > b),
            BinaryOp::GreaterEqual => compare_numeric(left, right, |a, b| a >= b),
            BinaryOp::And | BinaryOp::Or => unreachable!("logical operators short-circuit in evaluate"),
        }
    }
}

fn coerce(value: Value, expected: Type) -> Result<Value, String> {
    match (value, expected) {
        (Value::Integer(value), Type::Float) => Ok(Value::Float(value as f64)),
        (value, expected) if value_type(&value) == expected => Ok(value),
        (value, expected) => Err(format!(
            "cannot use runtime value of type {} as {expected}",
            type_name(&value)
        )),
    }
}

fn value_type(value: &Value) -> Type {
    match value {
        Value::Integer(_) => Type::Int,
        Value::Float(_) => Type::Float,
        Value::Bool(_) => Type::Bool,
        Value::String(_) => Type::String,
    }
}

fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Integer(a), Value::Integer(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Integer(a), Value::Float(b)) => *a as f64 == *b,
        (Value::Float(a), Value::Integer(b)) => *a == *b as f64,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        _ => false,
    }
}

fn compare_numeric(left: Value, right: Value, op: impl FnOnce(f64, f64) -> bool) -> Result<Value, String> {
    let left_type = type_name(&left);
    let right_type = type_name(&right);
    let a = as_number(left);
    let b = as_number(right);
    match (a, b) {
        (Some(a), Some(b)) => Ok(Value::Bool(op(a, b))),
        _ => Err(format!("comparison requires numbers, found {left_type} and {right_type}")),
    }
}

fn as_number(value: Value) -> Option<f64> {
    match value {
        Value::Integer(value) => Some(value as f64),
        Value::Float(value) => Some(value),
        _ => None,
    }
}

fn add(left: Value, right: Value) -> Result<Value, String> {
    match (left, right) {
        (Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),
        (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a + b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(a as f64 + b)),
        (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a + b as f64)),
        (a, b) => Err(format!(
            "operator '+' is not defined for {} and {}",
            type_name(&a),
            type_name(&b)
        )),
    }
}

fn numeric_binary(
    left: Value,
    right: Value,
    int_op: impl FnOnce(i64, i64) -> i64,
    float_op: impl FnOnce(f64, f64) -> f64,
) -> Result<Value, String> {
    match (left, right) {
        (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(int_op(a, b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(float_op(a, b))),
        (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(float_op(a as f64, b))),
        (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(float_op(a, b as f64))),
        (a, b) => Err(format!(
            "numeric operator is not defined for {} and {}",
            type_name(&a),
            type_name(&b)
        )),
    }
}

fn divide(left: Value, right: Value) -> Result<Value, String> {
    match (left, right) {
        (_, Value::Integer(0)) => Err("division by zero".into()),
        (_, Value::Float(value)) if value == 0.0 => Err("division by zero".into()),
        (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a / b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
        (Value::Integer(a), Value::Float(b)) => Ok(Value::Float(a as f64 / b)),
        (Value::Float(a), Value::Integer(b)) => Ok(Value::Float(a / b as f64)),
        (a, b) => Err(format!(
            "operator '/' is not defined for {} and {}",
            type_name(&a),
            type_name(&b)
        )),
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Integer(_) => "int",
        Value::Float(_) => "float",
        Value::Bool(_) => "bool",
        Value::String(_) => "string",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer::lex, parser::parse, typechecker};

    fn program(source: &str) -> Program {
        let program = parse(lex(source).unwrap()).unwrap();
        typechecker::check(&program).unwrap();
        program
    }

    #[test]
    fn executes_typed_function_call() {
        let program = program("fn add(a: int, b: int) -> int { return a + b; } fn main() {}");
        let mut interpreter = Interpreter::new(&program);
        let result = interpreter
            .call_function("add", vec![Value::Integer(2), Value::Integer(3)])
            .unwrap();
        assert_eq!(result, Some(Value::Integer(5)));
    }

    #[test]
    fn widens_int_argument_to_float() {
        let program = program("fn identity(x: float) -> float { return x; } fn main() {}");
        let mut interpreter = Interpreter::new(&program);
        let result = interpreter
            .call_function("identity", vec![Value::Integer(3)])
            .unwrap();
        assert_eq!(result, Some(Value::Float(3.0)));
    }

    #[test]
    fn executes_control_flow_with_functions() {
        let source = "fn inc(x: int) -> int { return x + 1; } fn main() { mut x: int = 0; while x < 3 { x = inc(x); } print(x); }";
        let program = program(source);
        assert!(execute(&program).is_ok());
    }
}
