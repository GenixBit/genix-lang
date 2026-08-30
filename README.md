# Genix

**Genix** is a modern programming language by **GenixBit**. Genix source files use the **`.gb`** extension.

> Status: **pre-alpha / v0.0.1**. Syntax, runtime behavior, standard-library APIs, and compiler interfaces may change.

## Create, run, inspect, and compile

```bash
gb new hello-genix
cd hello-genix
gb run
gb check
gb ir
```

Native builds use the separate **Genix Runtime**, and official library imports use **Genix Stdlib**. During source development, point the compiler at both repositories/installations:

```bash
export GENIX_RUNTIME=/path/to/genix-runtime
export GENIX_STDLIB=/path/to/genix-stdlib

gb build
./build/hello-genix
```

Release build:

```bash
gb build --release
```

## Compiler architecture

```text
Genix application
        +
Genix standard-library modules
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

## Standard library

The compiler now supports official standard-library modules. Import syntax is the same as project modules:

```gb
import io;
import math;
import string;

fn main() {
    io.println("Genix standard library");

    let value: int = math.abs_int(-42);
    io.print_int(value);

    let message: string = string.concat("Hello ", "Genix");
    io.println(message);
}
```

Module lookup order is:

```text
1. Project-local module beside the project entry source
2. Official module in GENIX_STDLIB/modules/
```

This means a local module can intentionally override a standard-library module during development.

Before loading an official module, the compiler validates `genix-stdlib/COMPATIBILITY`. The current compiler expects:

```text
GENIX_LANGUAGE_VERSION=0.0.1
GENIX_RUNTIME_ABI=1
```

Initial implemented stdlib modules are:

- `io` — typed output helpers
- `math` — basic numeric utilities
- `string` — concatenation and equality helpers

OS-facing modules such as `process`, `fs`, and `net` will be added after the compiler intrinsic/FFI contract is formalized.

## Genix Runtime integration

Native code targets:

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

The compiler checks `GENIX_RUNTIME` first and can also discover nearby runtime directories in common development layouts.

## Inspect Genix IR

```bash
gb ir
gb ir path/to/project
gb ir examples/functions.gb
```

IR carries resolved types, module-qualified calls, inferred variable types, and explicit safe casts such as `int → float`.

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
gb check [target]              Check syntax, modules, stdlib, and types
gb ir [target]                 Print typed Genix IR
gb build [project] [--release] Build and link a native executable
gb version                     Show the current version
gb help                        Show help
```

## Language/toolchain features implemented

Current support includes:

- `.gb` source files
- `genix.toml` projects
- Project-local and official stdlib module resolution
- Stdlib language/runtime compatibility validation
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
- External `genix-stdlib` integration
- Cross-repository CI across compiler, runtime, and stdlib

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
- Runtime and stdlib must currently be available locally during source development/native builds.
- Cross-compilation and target triples are not implemented yet.
- Runtime string representation is temporary.
- The final ownership/memory-safety model is not yet designed.
- Nested imports and package/registry imports are not implemented yet.
- Standard-library input/OS APIs still need a native intrinsic/FFI boundary.
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

With compiler/runtime/stdlib boundaries established, the next priorities are:

- Formal compiler intrinsic / native FFI layer
- `io.input` and `process` runtime APIs
- Stable toolchain installation and discovery
- Source-span diagnostics
- `gb test`
- `gb fmt`
- IR optimization passes
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
| Standard library | Genix Stdlib |

## Development

The compiler is implemented in **Rust**. The current runtime is portable **C11**. Most current stdlib modules are ordinary **Genix `.gb` source**.

```bash
export GENIX_RUNTIME=/path/to/genix-runtime
export GENIX_STDLIB=/path/to/genix-stdlib
cargo check
cargo test
cargo run -- check examples/stdlib
cargo run -- run examples/stdlib
cargo run -- ir examples/stdlib
cargo run -- build examples/stdlib
./examples/stdlib/build/stdlib-demo
```

GitHub Actions checks out `genix-lang`, `genix-runtime`, and `genix-stdlib`, validates each layer, and executes stdlib-backed native binaries.

## Project status

Genix is experimental and **not yet suitable for production use**. Breaking changes should be expected before the first stable release.

---

**Genix — by GenixBit**
