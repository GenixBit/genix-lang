use std::collections::HashMap;
use std::fmt;

use crate::ast::{BinaryOp, Expr, Program, Stmt, UnaryOp};

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
}

pub fn execute(program: &Program) -> Result<(), String> {
    let mut interpreter = Interpreter::new();
    interpreter.run(program)
}

struct Interpreter {
    scopes: Vec<HashMap<String, Binding>>,
}

impl Interpreter {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    fn run(&mut self, program: &Program) -> Result<(), String> {
        for statement in &program.body {
            self.execute_statement(statement)?;
        }
        Ok(())
    }

    fn execute_statement(&mut self, statement: &Stmt) -> Result<(), String> {
        match statement {
            Stmt::Let { name, value, mutable } => {
                let evaluated = self.evaluate(value)?;
                let scope = self.scopes.last_mut().expect("interpreter always has a scope");
                if scope.contains_key(name) {
                    return Err(format!("variable '{name}' is already declared in this scope"));
                }
                scope.insert(
                    name.clone(),
                    Binding {
                        value: evaluated,
                        mutable: *mutable,
                    },
                );
                Ok(())
            }
            Stmt::Assign { name, value } => {
                let evaluated = self.evaluate(value)?;
                self.assign(name, evaluated)
            }
            Stmt::Print(expr) => {
                println!("{}", self.evaluate(expr)?);
                Ok(())
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
                    Ok(())
                }
            }
            Stmt::While { condition, body } => {
                while self.expect_bool(self.evaluate(condition)?, "while condition")? {
                    self.execute_block(body)?;
                }
                Ok(())
            }
            Stmt::Block(body) => self.execute_block(body),
        }
    }

    fn execute_block(&mut self, body: &[Stmt]) -> Result<(), String> {
        self.scopes.push(HashMap::new());
        let result = (|| {
            for statement in body {
                self.execute_statement(statement)?;
            }
            Ok(())
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
                binding.value = value;
                return Ok(());
            }
        }
        Err(format!("undefined variable '{name}'"))
    }

    fn lookup(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn evaluate(&self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::Integer(value) => Ok(Value::Integer(*value)),
            Expr::Float(value) => Ok(Value::Float(*value)),
            Expr::Bool(value) => Ok(Value::Bool(*value)),
            Expr::String(value) => Ok(Value::String(value.clone())),
            Expr::Variable(name) => self
                .lookup(name)
                .map(|binding| binding.value.clone())
                .ok_or_else(|| format!("undefined variable '{name}'")),
            Expr::Unary { op, expr } => {
                let value = self.evaluate(expr)?;
                self.apply_unary(*op, value)
            }
            Expr::Binary { left, op, right } => match op {
                BinaryOp::And => {
                    let left = self.expect_bool(self.evaluate(left)?, "left side of '&&'")?;
                    if !left {
                        return Ok(Value::Bool(false));
                    }
                    let right = self.expect_bool(self.evaluate(right)?, "right side of '&&'")?;
                    Ok(Value::Bool(right))
                }
                BinaryOp::Or => {
                    let left = self.expect_bool(self.evaluate(left)?, "left side of '||'")?;
                    if left {
                        return Ok(Value::Bool(true));
                    }
                    let right = self.expect_bool(self.evaluate(right)?, "right side of '||'")?;
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
    use crate::{lexer::lex, parser::parse};

    #[test]
    fn executes_variables_and_arithmetic() {
        let source = "fn main() { let answer = 2 + 3 * 4; print(answer); }";
        let program = parse(lex(source).unwrap()).unwrap();
        assert!(execute(&program).is_ok());
    }

    #[test]
    fn executes_while_and_assignment() {
        let source = "fn main() { mut x = 0; while x < 3 { x = x + 1; } }";
        let program = parse(lex(source).unwrap()).unwrap();
        let mut interpreter = Interpreter::new();
        interpreter.run(&program).unwrap();
        assert_eq!(interpreter.lookup("x").unwrap().value, Value::Integer(3));
    }

    #[test]
    fn rejects_assignment_to_immutable_variable() {
        let source = "fn main() { let x = 1; x = 2; }";
        let program = parse(lex(source).unwrap()).unwrap();
        let error = execute(&program).unwrap_err();
        assert!(error.contains("immutable variable 'x'"));
    }

    #[test]
    fn evaluates_boolean_logic_and_if() {
        let source = "fn main() { mut x = 1; if x == 1 && !false { x = 2; } }";
        let program = parse(lex(source).unwrap()).unwrap();
        let mut interpreter = Interpreter::new();
        interpreter.run(&program).unwrap();
        assert_eq!(interpreter.lookup("x").unwrap().value, Value::Integer(2));
    }

    #[test]
    fn rejects_undefined_variables() {
        let source = "fn main() { print(missing); }";
        let program = parse(lex(source).unwrap()).unwrap();
        assert!(execute(&program).is_err());
    }
}
