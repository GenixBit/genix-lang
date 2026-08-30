# Genix

**Genix** is a modern programming language by **GenixBit**. Genix source files use the **`.gb`** extension.

> Status: **pre-alpha / v0.0.1**. Syntax, runtime behavior, and compiler interfaces may change.

## Create, run, inspect, and compile

```bash
gb new hello-genix
cd hello-genix
gb run
gb check
gb ir
```

Native builds now use the separate **Genix Runtime** repository. Point the compiler at it:

```bash
export GENIX_RUNTIME=/path/to/genix-runtime
gb build
./build/hello-genix
```

Release build:

```bash
gb build --release
```

## Compiler architecture

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
C11 backend
        ↓
Generated application C
        +
Genix Runtime ABI
        ↓
cc / clang / gcc
        ↓
Native executable
```

The C backend consumes typed Genix IR, not the parser AST.

## Genix Runtime integration

Native code now targets the public runtime header:

```text
genix-runtime/include/genix/runtime.h
```

and links the current portable implementation:

```text
genix-runtime/src/runtime.c
```

Runtime ABI v1 currently provides:

- Program startup/shutdown
- Tracked allocation
- Runtime panic handling
- String concatenation
- String equality
- Typed printing for `int`, `float`, `bool`, and `string`

Generated code uses calls such as:

```text
gb_runtime_init()
gb_string_concat(...)
gb_string_equal(...)
gb_print_string(...)
gb_runtime_shutdown()
```

The compiler first checks the `GENIX_RUNTIME` environment variable. It can also discover a nearby `genix-runtime` directory in common sibling/current-directory layouts.

If the runtime cannot be found, `gb build` fails with a clear setup error rather than silently embedding a private runtime copy.

## Inspect Genix IR

```bash
gb ir
gb ir path/to/project
gb ir examples/functions.gb
```

IR carries resolved types, module-qualified calls, inferred variable types, and explicit safe casts such as `int → float`, keeping backend code generation simpler and deterministic.

## Native build modes

Debug:

```bash
gb build
```

uses `-O0 -g`.

Release:

```bash
gb build --release
```

uses `-O2`.

The compiler checks `CC` first, then tries `cc`, `clang`, and `gcc`.

## Developer CLI

```text
gb new <name>                  Create a new Genix project
gb run [target]                Run through the interpreter
gb check [target]              Check syntax, modules, and types
gb ir [target]                 Print typed Genix IR
gb build [project] [--release] Build and link a native executable
gb version                     Show the current version
gb help                        Show help
```

## Language features implemented

Current support includes:

- `.gb` source files
- `genix.toml` projects
- Typed Genix IR
- Explicit IR numeric widening casts
- Multi-file modules and namespaced calls
- Multiple user-defined functions
- Typed parameters and return values
- `int`, `float`, `string`, `bool`
- Type inference and explicit annotations
- Static argument/return checking
- Safe `int → float` widening
- Immutable `let` and mutable `mut`
- Arithmetic, comparisons, and boolean logic
- `if` / `else`
- `while`
- Lexical block scope
- String concatenation/equality through the runtime
- `print(...)` through the runtime ABI
- Native debug and release builds
- External `genix-runtime` integration
- Automated cross-repository CI

## Native type mapping

| Genix | Runtime/native ABI |
|---|---|
| `int` | `int64_t` |
| `float` | `double` |
| `bool` | `bool` |
| `string` | `const char*` |
| void | `void` |

## Current limitations

- Native builds currently target the host platform only.
- A C compiler is required by the bootstrap C11 backend.
- `genix-runtime` must currently be available locally for native builds.
- Cross-compilation and target triples are not implemented yet.
- Runtime string representation is temporary.
- The final ownership/memory-safety model is not yet designed.
- Nested imports and package/registry imports are not implemented yet.
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

## Next compiler milestones

With the runtime boundary established, the next priorities are:

- Compiler/runtime ABI compatibility checking
- Stable runtime discovery/installation
- Source-span diagnostics
- IR optimization passes
- `gb test`
- `gb fmt`
- Standard-library integration
- Target triples and cross-compilation
- Memory-safety / ownership model
- LLVM backend
- WebAssembly backend
- Package management and lockfiles
- Concurrency / async
- Language server and editor tooling

## Ecosystem

- `genix-lang` — compiler and core language implementation
- `genix-runtime` — runtime ABI and low-level system support
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
| Runtime ABI | Genix Runtime ABI |

## Development

The compiler is implemented in **Rust**. The current runtime is portable **C11**.

```bash
export GENIX_RUNTIME=/path/to/genix-runtime
cargo check
cargo test
cargo run -- ir examples/project
cargo run -- build examples/project
./examples/project/build/module-demo
```

GitHub Actions checks out both `genix-lang` and `genix-runtime`, tests each layer, verifies generated code targets the runtime ABI, and executes debug/release native binaries.

## Project status

Genix is experimental and **not yet suitable for production use**. Breaking changes should be expected before the first stable release.

---

**Genix — by GenixBit**
