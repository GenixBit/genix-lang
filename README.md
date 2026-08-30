# Genix

**Genix** is a modern programming language by **GenixBit**, designed for expressive application development, AI-native software, backend services, automation, and high-performance tooling.

Genix source files use the **`.gb`** extension.

> Status: **pre-alpha / v0.0.1**. Syntax and APIs are not stable yet.

## Try the current interpreter

```gb
fn main() {
    let company = "GenixBit";
    let answer = 2 + 3 * 4;

    print("Hello from Genix!");
    print(company);
    print(answer);
}
```

Run it with:

```bash
cargo run -- run examples/basics.gb
```

Or, after installing/building the `gb` binary:

```bash
gb run examples/basics.gb
```

Validate syntax without executing:

```bash
gb check examples/basics.gb
```

## What works today

The first executable language pipeline is now implemented:

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
- `let` variables
- Integers
- Floating-point numbers
- Strings
- Booleans (`true` / `false`)
- Variable references
- `+`, `-`, `*`, `/`
- Parenthesized expressions
- Unary negative values
- String concatenation with `+`
- `print(...)`
- `//` line comments
- Basic lexer/parser/runtime diagnostics
- `gb run`
- `gb check`
- `gb version`

## Example

```gb
fn main() {
    let language = "Genix";
    let version = 0.1;
    let ready = true;
    let result = 10 + 5 * 2;

    print("Language: " + language);
    print(version);
    print(ready);
    print(result);
}
```

## Next language milestones

Development will proceed incrementally rather than adding large frameworks before the core language is stable.

### Milestone 2 — Control flow

- Comparison operators (`==`, `!=`, `<`, `<=`, `>`, `>=`)
- Logical operators (`&&`, `||`, `!`)
- `if` / `else`
- `while`
- Blocks and lexical scope

### Milestone 3 — Functions and types

- User-defined functions
- Parameters and return values
- Explicit types (`int`, `float`, `string`, `bool`)
- Static type checking
- Better compiler diagnostics

### Milestone 4 — Modules and tooling

- Imports/modules
- Project manifests
- `gb build`
- `gb test`
- `gb fmt`
- Package foundations

### Later

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

The initial implementation is written in **Rust**. The current execution engine is an interpreter, allowing the syntax and semantics to mature before introducing native code generation.

Run checks locally with:

```bash
cargo check
cargo test
cargo run -- run examples/hello.gb
```

GitHub Actions also runs the core compiler tests on pushes and pull requests.

## Project status

Genix is experimental and **not yet suitable for production use**. Breaking changes should be expected before the first stable release.

---

**Genix — by GenixBit**
