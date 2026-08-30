# Genix

**Genix** is a modern programming language by **GenixBit**, designed for expressive application development, AI-native software, backend services, automation, and high-performance tooling.

Genix source files use the **`.gb`** extension.

> Status: **pre-alpha / v0.0.1**. Syntax and APIs are not stable yet.

## Try Genix

```gb
fn main() {
    let company = "GenixBit";
    mut count = 0;

    while count < 5 {
        count = count + 1;

        if count == 3 {
            print("Genix reached three");
        } else {
            print(count);
        }
    }

    if count >= 5 && !false {
        print(company + " control flow works!");
    }
}
```

Run it with:

```bash
cargo run -- run examples/control_flow.gb
```

Or, after installing/building the `gb` binary:

```bash
gb run examples/control_flow.gb
```

Validate syntax without executing:

```bash
gb check examples/control_flow.gb
```

## What works today

The executable language pipeline is:

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
Interpreter
    ↓
Program output
```

Current language support includes:

- `.gb` source files
- `fn main()` entry point
- Immutable variables with `let`
- Mutable variables with `mut`
- Variable assignment
- Integers, floats, strings, and booleans
- Variable references
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
- Basic lexer/parser/runtime diagnostics
- `gb run`
- `gb check`
- `gb version`

## Mutability

Genix variables are immutable by default:

```gb
let language = "Genix";
```

Use `mut` only when a variable needs to change:

```gb
mut count = 0;
count = count + 1;
```

Assigning to a `let` variable produces a runtime error in the current interpreter. Static detection is planned as part of the type-checking milestone.

## Next language milestone — Functions and types

The next major compiler milestone is user-defined functions and static typing:

```gb
fn add(a: int, b: int) -> int {
    return a + b;
}

fn main() {
    let result: int = add(10, 20);
    print(result);
}
```

Planned work:

- User-defined functions
- Parameters
- Return values
- `return`
- Explicit types (`int`, `float`, `string`, `bool`)
- Function-call expressions
- Static type checking
- Compile-time mutability checks
- Improved diagnostics

## Later milestones

### Modules and tooling

- Imports/modules
- Project manifests
- `gb build`
- `gb test`
- `gb fmt`
- Package foundations

### Native platform

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
```

GitHub Actions runs compiler checks, tests, and executable Genix examples on pushes and pull requests.

## Project status

Genix is experimental and **not yet suitable for production use**. Breaking changes should be expected before the first stable release.

---

**Genix — by GenixBit**
