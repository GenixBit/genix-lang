# Genix

**Genix** is a modern programming language by **GenixBit**, designed for expressive application development, AI-native software, backend services, automation, and high-performance tooling.

Genix source files use the **`.gb`** extension.

> Status: **pre-alpha / v0.0.1**. Syntax and APIs are not stable yet.

## Try Genix

```gb
fn add(a: int, b: int) -> int {
    return a + b;
}

fn greet(name: string) {
    print("Hello " + name);
}

fn main() {
    let total: int = add(10, 20);
    greet("GenixBit");
    print(total);
}
```

Run it with:

```bash
cargo run -- run examples/functions.gb
```

Or, after building/installing the `gb` binary:

```bash
gb run examples/functions.gb
```

Validate syntax and types without executing:

```bash
gb check examples/functions.gb
```

## Compiler pipeline

The current executable pipeline is:

```text
.gb source
    ↓
Lexer
    ↓
Tokens
    ↓
Parser
    ↓
AST
    ↓
Static Type Checker
    ↓
Interpreter
    ↓
Program output
```

## What works today

Current language support includes:

- `.gb` source files
- Multiple user-defined functions
- `fn main()` entry point
- Function parameters
- Function calls
- `return`
- Return types with `->`
- Explicit types: `int`, `float`, `string`, `bool`
- Optional variable type annotations
- Static type checking before execution
- Function argument and return-value checking
- Guaranteed-return checks for non-void functions
- Safe `int` → `float` widening
- Immutable variables with `let`
- Mutable variables with `mut`
- Compile-time mutability checks
- Variable assignment
- Integers, floats, strings, and booleans
- Arithmetic: `+`, `-`, `*`, `/`
- Comparisons: `==`, `!=`, `<`, `<=`, `>`, `>=`
- Boolean logic: `&&`, `||`, `!`
- `if` / `else`
- `while`
- Lexical block scope
- Parenthesized expressions
- Unary negative values
- String concatenation with `+`
- `print(...)`
- `//` line comments
- Lexer/parser/type/runtime diagnostics
- `gb run`
- `gb check`
- `gb version`

## Functions

```gb
fn multiply(a: int, b: int) -> int {
    return a * b;
}

fn main() {
    let result: int = multiply(6, 7);
    print(result);
}
```

Functions without a return type are void functions:

```gb
fn greet(name: string) {
    print("Hello " + name);
}
```

## Static types

Genix currently supports four value types:

```text
int
float
string
bool
```

Variables may infer their type:

```gb
let age = 25;
```

or declare it explicitly:

```gb
let age: int = 25;
mut score: float = 10;
```

A mismatch is rejected before execution:

```gb
let age: int = "twenty";
```

Example diagnostic:

```text
Genix error: type error: initializer for 'age' expected int, found string
```

Genix permits safe widening from `int` to `float`:

```gb
fn scale(value: float) -> float {
    return value * 2.0;
}

fn main() {
    let result: float = scale(3);
    print(result);
}
```

## Mutability

Variables are immutable by default:

```gb
let language = "Genix";
```

Use `mut` when the value must change:

```gb
mut count: int = 0;
count = count + 1;
```

Assigning to a `let` variable is rejected by the static type-checking pass.

## Examples

```text
examples/
├── hello.gb
├── basics.gb
├── control_flow.gb
└── functions.gb
```

## Next milestone — Modules and project tooling

The next major milestone is moving from single-file programs toward real Genix projects:

```gb
import math;
import user;
```

Planned work:

- Modules/imports
- Multi-file `.gb` projects
- `genix.toml` project manifest
- `gb new`
- `gb build`
- `gb test`
- `gb fmt`
- Better source diagnostics
- Foundation for packages

## Later milestones

- Native code generation
- Memory-safety model
- Concurrency / async
- Standard library integration
- Web/backend APIs
- AI-native primitives
- Package registry
- Language server and editor tooling

## Repository scope

This is the flagship Genix language repository and contains the compiler/interpreter frontend and early developer tooling.

```text
src/
├── ast.rs
├── lexer.rs
├── parser.rs
├── typechecker.rs
├── interpreter.rs
└── main.rs
```

## Ecosystem

- `genix-lang` — compiler and core language implementation
- `genix-runtime` — runtime and system support
- `genix-stdlib` — official standard library
- `genix-docs` — specification and developer documentation
- `genix-site` — official website and developer portal

## Naming

| Item | Name |
|---|---|
| Language | Genix |
| Company | GenixBit |
| Source extension | `.gb` |
| CLI | `gb` |
| Compiler | `gbc` (planned native compiler) |

## Development

The initial implementation is written in **Rust**. The current execution engine is an interpreter, allowing syntax and semantics to mature before native code generation is introduced.

Run checks locally with:

```bash
cargo check
cargo test
cargo run -- run examples/hello.gb
cargo run -- run examples/control_flow.gb
cargo run -- run examples/functions.gb
```

GitHub Actions runs compiler checks, tests, and executable Genix examples on pushes and pull requests.

## Project status

Genix is experimental and **not yet suitable for production use**. Breaking changes should be expected before the first stable release.

---

**Genix — by GenixBit**
