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

pub fn execute(program: &Program) -> Result<(), String> {
    let mut interpreter = Interpreter::default();
    interpreter.run(program)
}

#[derive(Default)]
struct Interpreter {
    variables: HashMap<String, Value>,
}

impl Interpreter {
    fn run(&mut self, program: &Program) -> Result<(), String> {
        for statement in &program.body {
            self.execute_statement(statement)?;
        }
        Ok(())
    }

    fn execute_statement(&mut self, statement: &Stmt) -> Result<(), String> {
        match statement {
            Stmt::Let { name, value } => {
                let evaluated = self.evaluate(value)?;
                self.variables.insert(name.clone(), evaluated);
                Ok(())
            }
            Stmt::Print(expr) => {
                println!("{}", self.evaluate(expr)?);
                Ok(())
            }
        }
    }

    fn evaluate(&self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::Integer(value) => Ok(Value::Integer(*value)),
            Expr::Float(value) => Ok(Value::Float(*value)),
            Expr::Bool(value) => Ok(Value::Bool(*value)),
            Expr::String(value) => Ok(Value::String(value.clone())),
            Expr::Variable(name) => self
                .variables
                .get(name)
                .cloned()
                .ok_or_else(|| format!("undefined variable '{name}'")),
            Expr::Unary { op, expr } => {
                let value = self.evaluate(expr)?;
                self.apply_unary(*op, value)
            }
            Expr::Binary { left, op, right } => {
                let left = self.evaluate(left)?;
                let right = self.evaluate(right)?;
                self.apply_binary(left, *op, right)
            }
        }
    }

    fn apply_unary(&self, op: UnaryOp, value: Value) -> Result<Value, String> {
        match (op, value) {
            (UnaryOp::Negate, Value::Integer(value)) => Ok(Value::Integer(-value)),
            (UnaryOp::Negate, Value::Float(value)) => Ok(Value::Float(-value)),
            (UnaryOp::Negate, other) => Err(format!("cannot negate value of type {}", type_name(&other))),
        }
    }

    fn apply_binary(&self, left: Value, op: BinaryOp, right: Value) -> Result<Value, String> {
        match op {
            BinaryOp::Add => add(left, right),
            BinaryOp::Subtract => numeric_binary(left, right, |a, b| a - b, |a, b| a - b),
            BinaryOp::Multiply => numeric_binary(left, right, |a, b| a * b, |a, b| a * b),
            BinaryOp::Divide => divide(left, right),
        }
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
    fn rejects_undefined_variables() {
        let source = "fn main() { print(missing); }";
        let program = parse(lex(source).unwrap()).unwrap();
        assert!(execute(&program).is_err());
    }
}
