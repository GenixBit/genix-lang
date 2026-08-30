# Genix

**Genix** is a modern programming language by **GenixBit**, designed for expressive application development, AI-native software, backend services, automation, and high-performance tooling.

Genix source files use the **`.gb`** extension.

> Status: **pre-alpha / v0.0.1**. Syntax and APIs are not stable yet.

## Create a Genix project

```bash
gb new hello-genix
cd hello-genix
gb run
```

A project uses this layout:

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

## Multi-file modules

Genix projects can import `.gb` modules from the entry file's source directory.

```text
src/
├── main.gb
├── math.gb
└── greeting.gb
```

`src/math.gb`:

```gb
fn add(a: int, b: int) -> int {
    return a + b;
}

fn twice(value: int) -> int {
    return add(value, value);
}
```

`src/greeting.gb`:

```gb
fn hello(name: string) {
    print("Hello " + name);
}
```

`src/main.gb`:

```gb
import math;
import greeting;

fn main() {
    let answer: int = math.twice(21);
    greeting.hello("Genix");
    print(answer);
}
```

Run the project:

```bash
gb run
```

Or target a project directory explicitly:

```bash
gb run examples/project
```

## Developer CLI

```text
gb new <name>        Create a new Genix project
gb run [target]      Run a .gb file or project
gb check [target]    Check syntax, modules, and types
gb build [project]   Produce a checked frontend build artifact
gb version           Show the current version
gb help              Show help
```

`gb run` and `gb check` default to the current project when no target is supplied.

`gb build` currently produces `build/genix.frontend`. This proves project loading, module resolution, parsing, and static type checking succeeded. **Native executable generation is not implemented yet** and is the next backend milestone.

## Compiler pipeline

```text
Genix project / .gb source
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
Interpreter
        ↓
Program output
```

## What works today

Current support includes:

- `.gb` source files
- `genix.toml` project manifests
- `gb new`
- Project-level `gb run`
- Project-level `gb check`
- Frontend `gb build`
- Multi-file modules with `import module;`
- Namespaced calls such as `math.add(...)`
- Internal function calls within imported modules
- Multiple user-defined functions
- `fn main()` entry point
- Function parameters and calls
- `return`
- Return types with `->`
- Static types: `int`, `float`, `string`, `bool`
- Type inference and explicit annotations
- Static argument and return-value checking
- Guaranteed-return checks for non-void functions
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
- Automated GitHub CI

### Current module limitation

The first module-system version intentionally keeps resolution simple: imports are declared in the project entry file and map to sibling files such as `src/math.gb`. Nested imports inside imported modules are not supported yet.

## Single-file programs

Single `.gb` files still work:

```bash
gb run examples/functions.gb
gb check examples/functions.gb
```

Example:

```gb
fn add(a: int, b: int) -> int {
    return a + b;
}

fn main() {
    let total: int = add(10, 20);
    print(total);
}
```

## Examples

```text
examples/
├── hello.gb
├── basics.gb
├── control_flow.gb
├── functions.gb
└── project/
    ├── genix.toml
    └── src/
        ├── main.gb
        ├── math.gb
        └── greeting.gb
```

## Next milestone — Native compiler backend

The next major milestone is moving beyond interpretation and frontend artifacts toward real compilation.

Planned work:

- Genix intermediate representation (IR)
- `gb build` native executable output
- LLVM or another native backend
- Debug/release build modes
- Target triples
- Better source-span diagnostics
- Runtime integration

After the backend foundation, work can expand into:

- `gb test`
- `gb fmt`
- Package management
- Standard library integration
- Concurrency / async
- Memory-safety model
- AI-native primitives
- Language server and editor tooling

## Repository scope

```text
src/
├── ast.rs
├── lexer.rs
├── parser.rs
├── typechecker.rs
├── interpreter.rs
├── project.rs
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

The implementation is written in **Rust**. Run the current checks with:

```bash
cargo check
cargo test
cargo run -- run examples/functions.gb
cargo run -- run examples/project
cargo run -- build examples/project
```

GitHub Actions validates the compiler, language tests, single-file examples, module project execution, project builds, and `gb new` smoke tests on pushes and pull requests.

## Project status

Genix is experimental and **not yet suitable for production use**. Breaking changes should be expected before the first stable release.

---

**Genix — by GenixBit**
