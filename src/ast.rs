use std::fmt;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    Void,
    OptionInt,
    OptionFloat,
    OptionBool,
    OptionString,
    ResultInt,
    ResultFloat,
    ResultBool,
    ResultString,
}

impl Type {
    pub fn option(inner: Type) -> Option<Type> {
        match inner {
            Type::Int => Some(Type::OptionInt),
            Type::Float => Some(Type::OptionFloat),
            Type::Bool => Some(Type::OptionBool),
            Type::String => Some(Type::OptionString),
            _ => None,
        }
    }

    pub fn result(ok: Type, error: Type) -> Option<Type> {
        if error != Type::String {
            return None;
        }
        match ok {
            Type::Int => Some(Type::ResultInt),
            Type::Float => Some(Type::ResultFloat),
            Type::Bool => Some(Type::ResultBool),
            Type::String => Some(Type::ResultString),
            _ => None,
        }
    }

    pub fn option_inner(self) -> Option<Type> {
        match self {
            Type::OptionInt => Some(Type::Int),
            Type::OptionFloat => Some(Type::Float),
            Type::OptionBool => Some(Type::Bool),
            Type::OptionString => Some(Type::String),
            _ => None,
        }
    }

    pub fn result_ok(self) -> Option<Type> {
        match self {
            Type::ResultInt => Some(Type::Int),
            Type::ResultFloat => Some(Type::Float),
            Type::ResultBool => Some(Type::Bool),
            Type::ResultString => Some(Type::String),
            _ => None,
        }
    }

    pub fn is_result(self) -> bool {
        self.result_ok().is_some()
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::Float => write!(f, "float"),
            Type::Bool => write!(f, "bool"),
            Type::String => write!(f, "string"),
            Type::Void => write!(f, "void"),
            Type::OptionInt => write!(f, "Option<int>"),
            Type::OptionFloat => write!(f, "Option<float>"),
            Type::OptionBool => write!(f, "Option<bool>"),
            Type::OptionString => write!(f, "Option<string>"),
            Type::ResultInt => write!(f, "Result<int,string>"),
            Type::ResultFloat => write!(f, "Result<float,string>"),
            Type::ResultBool => write!(f, "Result<bool,string>"),
            Type::ResultString => write!(f, "Result<string,string>"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        name: String,
        value: Expr,
        mutable: bool,
        annotation: Option<Type>,
    },
    Assign {
        name: String,
        value: Expr,
    },
    Print(Expr),
    Expr(Expr),
    Return(Option<Expr>),
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    Match {
        value: Expr,
        arms: Vec<MatchArm>,
    },
    Block(Vec<Stmt>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    Some(String),
    None,
    Ok(String),
    Err(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Integer(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Variable(String),
    Call {
        callee: String,
        arguments: Vec<Expr>,
    },
    Some(Box<Expr>),
    None,
    Ok(Box<Expr>),
    Err(Box<Expr>),
    Try(Box<Expr>),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}
