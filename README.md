# Genix

**Genix** is a modern programming language by **GenixBit**. Genix source files use the **`.gb`** extension.

> Status: **pre-alpha / v0.0.1**. Syntax, runtime behavior, and compiler interfaces may change.

## Create, run, inspect, and compile a Genix project

```bash
gb new hello-genix
cd hello-genix
gb run
gb check
gb ir
gb build
./build/hello-genix
```

Release build:

```bash
gb build --release
./build/hello-genix
```

A project uses:

```text
hello-genix/
├── genix.toml
└── src/
    └── main.gb
```

## Compiler architecture

Genix now has a backend-neutral, **typed intermediate representation (Genix IR)** between the frontend and native code generation.

```text
Genix project / .gb files
        ↓
Project + module loader
        ↓
Lexer
        ↓
Parser
        ↓
AST
        ↓
Static Type Checker
        ↓
Typed Genix IR
        ↓
Backend
        ├── C11 backend (implemented)
        ├── LLVM backend (planned)
        └── WebAssembly backend (planned)
        ↓
Native / target output
```

The IR resolves information that backends should not have to rediscover:

- Function signatures
- Variable types, including inferred types
- Module-qualified function names
- Expression result types
- Explicit safe numeric widening casts such as `int → float`
- Structured control flow

This keeps language semantics in the frontend/IR layer and makes backends primarily responsible for target code generation.

## Inspect Genix IR

Use:

```bash
gb ir
```

or:

```bash
gb ir path/to/project
gb ir examples/functions.gb
```

For example, source such as:

```gb
fn average(a: float, b: float) -> float {
    return (a + b) / 2.0;
}

fn main() {
    let result: float = average(10, 20);
    print(result);
}
```

is lowered to typed IR where the integer arguments are represented with explicit `cast<float>(...)` nodes before the backend sees them.

## Native compilation

`gb build` produces a real host-native executable from Genix IR.

```text
Typed Genix IR
      ↓
C11 code generator
      ↓
Generated C source
      ↓
cc / clang / gcc
      ↓
Native executable
```

Example output:

```text
build/
├── hello-genix.c
└── hello-genix
```

Debug build:

```bash
gb build
```

uses `-O0 -g`.

Release build:

```bash
gb build --release
```

uses `-O2`.

The compiler checks the `CC` environment variable first, then tries `cc`, `clang`, and `gcc`.

## Multi-file modules

```text
src/
├── main.gb
├── math.gb
└── greeting.gb
```

`math.gb`:

```gb
fn add(a: int, b: int) -> int {
    return a + b;
}

fn twice(value: int) -> int {
    return add(value, value);
}
```

`main.gb`:

```gb
import math;

fn main() {
    let answer: int = math.twice(21);
    print(answer);
}
```

Imported functions are represented internally with names such as `math.twice`, giving the IR and backends deterministic module-qualified symbols.

## Developer CLI

```text
gb new <name>                  Create a new Genix project
gb run [target]                Run through the interpreter
gb check [target]              Check syntax, modules, and types
gb ir [target]                 Print typed Genix IR
gb build [project] [--release] Build a native executable from IR
gb version                     Show the current version
gb help                        Show help
```

## Language features implemented

Current support includes:

- `.gb` source files
- `genix.toml` projects
- Typed Genix IR
- Explicit IR numeric widening casts
- `gb new`, `gb run`, `gb check`, `gb ir`, and `gb build`
- Debug and release native builds
- C11 native backend
- Multi-file modules with `import module;`
- Namespaced calls such as `math.add(...)`
- Multiple user-defined functions
- Typed parameters and return values
- `return`
- Static types: `int`, `float`, `string`, `bool`
- Type inference and explicit annotations
- Static function argument and return checking
- Guaranteed-return checking
- Safe `int` → `float` widening
- Immutable `let` and mutable `mut`
- Arithmetic, comparisons, and boolean logic
- `if` / `else`
- `while`
- Lexical block scope
- String concatenation
- `print(...)`
- `//` comments
- Automated CI that inspects IR and executes generated native binaries

## Native type mapping

| Genix | C11 backend |
|---|---|
| `int` | `int64_t` |
| `float` | `double` |
| `bool` | `bool` |
| `string` | `const char*` |
| void function | `void` |

## Current limitations

- Native builds currently target the host platform only.
- A C compiler is required for the current native backend.
- Cross-compilation and target triples are not implemented yet.
- The string runtime is intentionally minimal; the full memory-safety model is still to be designed.
- Nested imports inside imported modules are not supported yet.
- Package/registry imports are not implemented yet.
- LLVM and WebAssembly backends are not implemented yet.

## Repository architecture

```text
src/
├── ast.rs
├── lexer.rs
├── parser.rs
├── typechecker.rs
├── ir.rs
├── interpreter.rs
├── project.rs
├── codegen.rs
└── main.rs
```

The C backend consumes `ir::Program`, not the parser AST.

## Next compiler milestones

With the IR boundary established, the next compiler work should focus on:

- IR optimization passes
- Target triples and explicit target selection
- Cross-compilation architecture
- Runtime integration through `genix-runtime`
- Source-span diagnostics through AST → IR → backend
- `gb test`
- `gb fmt`
- Standard-library integration
- Package management and lockfiles
- Memory-safety / ownership model
- LLVM backend
- WebAssembly backend
- Concurrency / async
- Language server and editor tooling

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
| Compiler | `gbc` (planned standalone compiler identity) |
| Intermediate representation | Genix IR |

## Development

The compiler is implemented in **Rust**.

```bash
cargo check
cargo test
cargo run -- ir examples/project
cargo run -- build examples/project
./examples/project/build/module-demo
cargo run -- build examples/project --release
```

GitHub Actions validates the compiler frontend, interpreter, module system, typed IR, IR numeric casts, native debug builds, native release builds, and execution of generated binaries.

## Project status

Genix is experimental and **not yet suitable for production use**. Breaking changes should be expected before the first stable release.

---

**Genix — by GenixBit**
