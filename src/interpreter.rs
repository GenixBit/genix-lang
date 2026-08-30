use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::process as host_process;

use crate::ast::{BinaryOp, Expr, Function, MatchArm, Pattern, Program, Stmt, Type, UnaryOp};

const PROPAGATE_PREFIX: &str = "__GENIX_PROPAGATE_ERR__:";

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    Bool(bool),
    String(String),
    OptionSome(Box<Value>),
    OptionNone,
    ResultOk(Box<Value>),
    ResultErr(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(value) => write!(f, "{value}"),
            Value::Float(value) => write!(f, "{value}"),
            Value::Bool(value) => write!(f, "{value}"),
            Value::String(value) => write!(f, "{value}"),
            Value::OptionSome(value) => write!(f, "Some({value})"),
            Value::OptionNone => write!(f, "None"),
            Value::ResultOk(value) => write!(f, "Ok({value})"),
            Value::ResultErr(error) => write!(f, "Err({error})"),
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
        let functions = program.functions.iter().cloned()
            .map(|function| (function.name.clone(), function))
            .collect();
        Self { functions, scopes: vec![HashMap::new()] }
    }

    fn call_function(&mut self, name: &str, arguments: Vec<Value>) -> Result<Option<Value>, String> {
        if is_stdlib_intrinsic(name) {
            return self.call_stdlib_intrinsic(name, arguments);
        }

        let function = self.functions.get(name).cloned()
            .ok_or_else(|| format!("undefined function '{name}'"))?;
        if arguments.len() != function.params.len() {
            return Err(format!(
                "function '{name}' expects {} argument(s), found {}",
                function.params.len(), arguments.len()
            ));
        }

        let caller_scopes = std::mem::replace(&mut self.scopes, vec![HashMap::new()]);
        let result = (|| {
            for (param, value) in function.params.iter().zip(arguments.into_iter()) {
                let value = coerce(value, param.ty)?;
                self.scopes[0].insert(
                    param.name.clone(),
                    Binding { value, mutable: false, ty: param.ty },
                );
            }
            for statement in &function.body {
                match self.execute_statement(statement)? {
                    Flow::Continue => {}
                    Flow::Return(value) => return self.finish_return(&function, value),
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

        match result {
            Err(error) if error.starts_with(PROPAGATE_PREFIX) && function.return_type.is_result() => {
                Ok(Some(Value::ResultErr(error[PROPAGATE_PREFIX.len()..].to_string())))
            }
            other => other,
        }
    }

    fn call_stdlib_intrinsic(&mut self, name: &str, arguments: Vec<Value>) -> Result<Option<Value>, String> {
        match name {
            "io.input" => {
                require_arity(name, &arguments, 1)?;
                let prompt = expect_string(arguments.into_iter().next().unwrap(), name)?;
                print!("{prompt}");
                io::stdout().flush()
                    .map_err(|error| format!("io.input could not flush stdout: {error}"))?;
                let mut line = String::new();
                io::stdin().read_line(&mut line)
                    .map_err(|error| format!("io.input failed: {error}"))?;
                while line.ends_with('\n') || line.ends_with('\r') { line.pop(); }
                Ok(Some(Value::String(line)))
            }
            "process.env" => {
                require_arity(name, &arguments, 1)?;
                let key = expect_string(arguments.into_iter().next().unwrap(), name)?;
                Ok(Some(Value::String(std::env::var(key).unwrap_or_default())))
            }
            "process.env_option" => {
                require_arity(name, &arguments, 1)?;
                let key = expect_string(arguments.into_iter().next().unwrap(), name)?;
                Ok(Some(match std::env::var(key) {
                    Ok(value) => Value::OptionSome(Box::new(Value::String(value))),
                    Err(_) => Value::OptionNone,
                }))
            }
            "process.exit" => {
                require_arity(name, &arguments, 1)?;
                let code = expect_int(arguments.into_iter().next().unwrap(), name)?;
                host_process::exit(code as i32)
            }
            "fs.read_text" => {
                require_arity(name, &arguments, 1)?;
                let path = expect_string(arguments.into_iter().next().unwrap(), name)?;
                let text = fs::read_to_string(&path)
                    .map_err(|error| format!("fs.read_text('{path}') failed: {error}"))?;
                Ok(Some(Value::String(text)))
            }
            "fs.try_read_text" => {
                require_arity(name, &arguments, 1)?;
                let path = expect_string(arguments.into_iter().next().unwrap(), name)?;
                Ok(Some(match fs::read_to_string(&path) {
                    Ok(text) => Value::ResultOk(Box::new(Value::String(text))),
                    Err(error) => Value::ResultErr(format!("fs.try_read_text('{path}') failed: {error}")),
                }))
            }
            "fs.write_text" => {
                require_arity(name, &arguments, 2)?;
                let mut values = arguments.into_iter();
                let path = expect_string(values.next().unwrap(), name)?;
                let text = expect_string(values.next().unwrap(), name)?;
                fs::write(&path, text)
                    .map_err(|error| format!("fs.write_text('{path}') failed: {error}"))?;
                Ok(None)
            }
            "fs.try_write_text" => {
                require_arity(name, &arguments, 2)?;
                let mut values = arguments.into_iter();
                let path = expect_string(values.next().unwrap(), name)?;
                let text = expect_string(values.next().unwrap(), name)?;
                Ok(Some(match fs::write(&path, text) {
                    Ok(()) => Value::ResultOk(Box::new(Value::Bool(true))),
                    Err(error) => Value::ResultErr(format!("fs.try_write_text('{path}') failed: {error}")),
                }))
            }
            _ => Err(format!("unknown Genix stdlib intrinsic '{name}'")),
        }
    }

    fn finish_return(&self, function: &Function, value: Option<Value>) -> Result<Option<Value>, String> {
        match (function.return_type, value) {
            (Type::Void, None) => Ok(None),
            (Type::Void, Some(_)) => Err(format!("void function '{}' returned a value", function.name)),
            (expected, Some(value)) => Ok(Some(coerce(value, expected)?)),
            (expected, None) => Err(format!("function '{}' must return a value of type {expected}", function.name)),
        }
    }

    fn execute_statement(&mut self, statement: &Stmt) -> Result<Flow, String> {
        match statement {
            Stmt::Let { name, value, mutable, annotation } => {
                let evaluated = self.evaluate(value)?;
                let ty = match annotation {
                    Some(ty) => *ty,
                    None => value_type(&evaluated)
                        .ok_or_else(|| format!("runtime cannot infer type for variable '{name}'"))?,
                };
                let evaluated = coerce(evaluated, ty)?;
                let scope = self.scopes.last_mut().expect("interpreter always has a scope");
                if scope.contains_key(name) {
                    return Err(format!("variable '{name}' is already declared in this scope"));
                }
                scope.insert(name.clone(), Binding { value: evaluated, mutable: *mutable, ty });
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
                    Expr::Try(_) => { self.evaluate(expr)?; }
                    _ => { self.evaluate(expr)?; }
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
            Stmt::If { condition, then_branch, else_branch } => {
                let condition_value = self.evaluate(condition)?;
                if self.expect_bool(condition_value, "if condition")? {
                    self.execute_block(then_branch)
                } else if let Some(branch) = else_branch {
                    self.execute_block(branch)
                } else {
                    Ok(Flow::Continue)
                }
            }
            Stmt::While { condition, body } => {
                loop {
                    let condition_value = self.evaluate(condition)?;
                    if !self.expect_bool(condition_value, "while condition")? { break; }
                    match self.execute_block(body)? {
                        Flow::Continue => {}
                        returned @ Flow::Return(_) => return Ok(returned),
                    }
                }
                Ok(Flow::Continue)
            }
            Stmt::Match { value, arms } => {
                let value = self.evaluate(value)?;
                self.execute_match(value, arms)
            }
            Stmt::Block(body) => self.execute_block(body),
        }
    }

    fn execute_match(&mut self, value: Value, arms: &[MatchArm]) -> Result<Flow, String> {
        for arm in arms {
            let binding = match (&arm.pattern, &value) {
                (Pattern::Some(name), Value::OptionSome(inner)) => Some((name.clone(), (**inner).clone())),
                (Pattern::None, Value::OptionNone) => Some((String::new(), Value::Bool(false))),
                (Pattern::Ok(name), Value::ResultOk(inner)) => Some((name.clone(), (**inner).clone())),
                (Pattern::Err(name), Value::ResultErr(error)) => Some((name.clone(), Value::String(error.clone()))),
                _ => None,
            };
            if let Some((name, bound)) = binding {
                self.scopes.push(HashMap::new());
                if !name.is_empty() {
                    let ty = value_type(&bound).ok_or_else(|| "runtime could not determine match binding type".to_string())?;
                    self.scopes.last_mut().unwrap().insert(
                        name,
                        Binding { value: bound, mutable: false, ty },
                    );
                }
                let result = (|| {
                    for statement in &arm.body {
                        match self.execute_statement(statement)? {
                            Flow::Continue => {}
                            returned @ Flow::Return(_) => return Ok(returned),
                        }
                    }
                    Ok(Flow::Continue)
                })();
                self.scopes.pop();
                return result;
            }
        }
        Err("runtime error: exhaustive match had no matching arm".into())
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
                    return Err(format!("cannot assign to immutable variable '{name}'; declare it with 'mut'"));
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
        arguments.iter().map(|argument| self.evaluate(argument)).collect()
    }

    fn evaluate(&mut self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::Integer(value) => Ok(Value::Integer(*value)),
            Expr::Float(value) => Ok(Value::Float(*value)),
            Expr::Bool(value) => Ok(Value::Bool(*value)),
            Expr::String(value) => Ok(Value::String(value.clone())),
            Expr::Variable(name) => self.lookup(name)
                .map(|binding| binding.value.clone())
                .ok_or_else(|| format!("undefined variable '{name}'")),
            Expr::Call { callee, arguments } => {
                let values = self.evaluate_arguments(arguments)?;
                self.call_function(callee, values)?
                    .ok_or_else(|| format!("function '{callee}' does not return a value"))
            }
            Expr::Some(value) => Ok(Value::OptionSome(Box::new(self.evaluate(value)?))),
            Expr::None => Ok(Value::OptionNone),
            Expr::Ok(value) => Ok(Value::ResultOk(Box::new(self.evaluate(value)?))),
            Expr::Err(error) => {
                let value = self.evaluate(error)?;
                Ok(Value::ResultErr(expect_string(value, "Err")?))
            }
            Expr::Try(inner) => match self.evaluate(inner)? {
                Value::ResultOk(value) => Ok(*value),
                Value::ResultErr(error) => Err(format!("{PROPAGATE_PREFIX}{error}")),
                other => Err(format!("runtime '?': expected Result, found {}", type_name(&other))),
            },
            Expr::Unary { op, expr } => {
                let value = self.evaluate(expr)?;
                self.apply_unary(*op, value)
            }
            Expr::Binary { left, op, right } => match op {
                BinaryOp::And => {
                    let left_value = self.evaluate(left)?;
                    let left = self.expect_bool(left_value, "left side of '&&'")?;
                    if !left { return Ok(Value::Bool(false)); }
                    let right_value = self.evaluate(right)?;
                    Ok(Value::Bool(self.expect_bool(right_value, "right side of '&&'")?))
                }
                BinaryOp::Or => {
                    let left_value = self.evaluate(left)?;
                    let left = self.expect_bool(left_value, "left side of '||'")?;
                    if left { return Ok(Value::Bool(true)); }
                    let right_value = self.evaluate(right)?;
                    Ok(Value::Bool(self.expect_bool(right_value, "right side of '||'")?))
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

fn is_stdlib_intrinsic(name: &str) -> bool {
    matches!(
        name,
        "io.input" | "process.env" | "process.env_option" | "process.exit"
            | "fs.read_text" | "fs.try_read_text" | "fs.write_text" | "fs.try_write_text"
    )
}

fn require_arity(name: &str, arguments: &[Value], expected: usize) -> Result<(), String> {
    if arguments.len() == expected { Ok(()) } else {
        Err(format!("intrinsic '{name}' expects {expected} argument(s), found {}", arguments.len()))
    }
}

fn expect_string(value: Value, context: &str) -> Result<String, String> {
    match value {
        Value::String(value) => Ok(value),
        other => Err(format!("{context} expected string, found {}", type_name(&other))),
    }
}

fn expect_int(value: Value, context: &str) -> Result<i64, String> {
    match value {
        Value::Integer(value) => Ok(value),
        other => Err(format!("{context} expected int, found {}", type_name(&other))),
    }
}

fn coerce(value: Value, expected: Type) -> Result<Value, String> {
    match (value, expected) {
        (Value::Integer(value), Type::Float) => Ok(Value::Float(value as f64)),
        (Value::OptionNone, ty) if ty.option_inner().is_some() => Ok(Value::OptionNone),
        (Value::ResultErr(error), ty) if ty.result_ok().is_some() => Ok(Value::ResultErr(error)),
        (value, expected) if value_type(&value) == Some(expected) => Ok(value),
        (value, expected) => Err(format!(
            "cannot use runtime value of type {} as {expected}", type_name(&value)
        )),
    }
}

fn value_type(value: &Value) -> Option<Type> {
    match value {
        Value::Integer(_) => Some(Type::Int),
        Value::Float(_) => Some(Type::Float),
        Value::Bool(_) => Some(Type::Bool),
        Value::String(_) => Some(Type::String),
        Value::OptionSome(value) => Type::option(value_type(value)?),
        Value::OptionNone => None,
        Value::ResultOk(value) => Type::result(value_type(value)?, Type::String),
        Value::ResultErr(_) => None,
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
        (a, b) => Err(format!("operator '+' is not defined for {} and {}", type_name(&a), type_name(&b))),
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
        (a, b) => Err(format!("numeric operator is not defined for {} and {}", type_name(&a), type_name(&b))),
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
        (a, b) => Err(format!("operator '/' is not defined for {} and {}", type_name(&a), type_name(&b))),
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Integer(_) => "int",
        Value::Float(_) => "float",
        Value::Bool(_) => "bool",
        Value::String(_) => "string",
        Value::OptionSome(_) | Value::OptionNone => "Option",
        Value::ResultOk(_) | Value::ResultErr(_) => "Result",
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
    fn executes_option_match() {
        let source = "fn main() { let x: Option<string> = Some(\"Genix\"); match x { Some(v) => { print(v); } None => { print(\"none\"); } } }";
        assert!(execute(&program(source)).is_ok());
    }

    #[test]
    fn propagates_result_error() {
        let source = "fn fail() -> Result<string,string> { return Err(\"boom\"); } fn wrap() -> Result<string,string> { let x: string = fail()?; return Ok(x); } fn main() { let r: Result<string,string> = wrap(); match r { Ok(v) => { print(v); } Err(e) => { print(e); } } }";
        assert!(execute(&program(source)).is_ok());
    }
}
