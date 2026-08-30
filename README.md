# Genix

**Genix** is a modern programming language by **GenixBit**. Genix source files use the **`.gb`** extension.

> Status: **pre-alpha / v0.0.1**. Syntax, runtime behavior, and compiler interfaces may change.

## Create, run, and compile a Genix project

```bash
gb new hello-genix
cd hello-genix
gb run
gb check
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

`genix.toml`:

```toml
[project]
name = "hello-genix"
version = "0.1.0"
entry = "src/main.gb"
```

`src/main.gb`:

```gb
fn main() {
    print("Hello from Genix!");
}
```

## Native compilation

`gb build` now produces a **real host-native executable**.

Current backend pipeline:

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
Genix C11 Backend
        ↓
Generated C source
        ↓
System C compiler
        ↓
Native executable
```

Example output:

```text
build/
├── hello-genix.c
└── hello-genix
```

The backend currently looks for the `CC` environment variable and then `cc`, `clang`, or `gcc`.

Debug builds use `-O0 -g`:

```bash
gb build
```

Release builds use `-O2`:

```bash
gb build --release
```

The C11 backend is the first native backend. It gives Genix a working native compilation path while the compiler architecture evolves toward a dedicated IR and additional backends such as LLVM.

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

Imported functions are accessed through namespaces such as `math.twice(...)`. Internal calls within a module are automatically resolved to that module namespace.

## Developer CLI

```text
gb new <name>                  Create a new Genix project
gb run [target]                Run a .gb file or project through the interpreter
gb check [target]              Check syntax, modules, and static types
gb build [project] [--release] Build a native executable
gb version                     Show the current version
gb help                        Show help
```

`gb run` and `gb check` default to the current project when no target is supplied.

## Language features implemented

Current support includes:

- `.gb` source files
- `genix.toml` projects
- `gb new`, `gb run`, `gb check`, and native `gb build`
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
- Compile-time mutability checks
- Arithmetic: `+`, `-`, `*`, `/`
- Comparisons: `==`, `!=`, `<`, `<=`, `>`, `>=`
- Boolean logic: `&&`, `||`, `!`
- `if` / `else`
- `while`
- Lexical block scope
- String concatenation
- `print(...)`
- `//` comments
- Automated CI that builds and executes native Genix binaries

## Native type mapping

The first C11 backend maps Genix values to native C representations:

| Genix | C11 backend |
|---|---|
| `int` | `int64_t` |
| `float` | `double` |
| `bool` | `bool` |
| `string` | `const char*` |
| void function | `void` |

The generated runtime currently provides string concatenation and basic runtime failure handling.

## Current backend limitations

This is a bootstrap native compiler, not the final backend architecture.

- Native builds currently target the host platform only.
- A C compiler (`cc`, `clang`, or `gcc`) is required.
- Cross-compilation and target triples are not implemented yet.
- The generated string runtime is intentionally minimal; full ownership/lifetime memory management is still to be designed.
- Nested imports inside imported modules are not supported yet.
- Package/registry imports are not implemented yet.

## Single-file development

Single files can still be interpreted and checked directly:

```bash
gb run examples/functions.gb
gb check examples/functions.gb
```

Native `gb build` operates on projects with `genix.toml` so builds have a stable project name, entry point, and output directory.

## Repository architecture

```text
src/
├── ast.rs
├── lexer.rs
├── parser.rs
├── typechecker.rs
├── interpreter.rs
├── project.rs
├── codegen.rs
└── main.rs
```

## Next compiler milestones

The next compiler work should build on the now-working native path:

- Genix IR between type checking and backend generation
- Target triples and cross-compilation
- Separate runtime integration through `genix-runtime`
- Better source-span diagnostics
- `gb test`
- `gb fmt`
- Standard-library integration
- Package management and lockfiles
- Memory-safety / ownership model
- Concurrency / async
- LLVM backend
- Language server and editor tooling
- AI-native standard APIs

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

## Development

The compiler is implemented in **Rust**.

```bash
cargo check
cargo test
cargo run -- run examples/project
cargo run -- build examples/project
./examples/project/build/module-demo
cargo run -- build examples/project --release
```

GitHub Actions validates the Rust compiler, interpreter, modules, static type checker, project generator, native debug build, native release build, and execution of generated native binaries.

## Project status

Genix is experimental and **not yet suitable for production use**. Breaking changes should be expected before the first stable release.

---

**Genix — by GenixBit**
