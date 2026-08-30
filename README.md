# Genix

**Genix** is a modern programming language by **GenixBit**. Genix source files use the **`.gb`** extension.

> Status: **pre-alpha / v0.0.1**. Syntax, runtime behavior, standard-library APIs, and compiler interfaces may change.

## Create, run, inspect, and compile

```bash
gb new hello-genix
cd hello-genix

export GENIX_RUNTIME=/path/to/genix-runtime
export GENIX_STDLIB=/path/to/genix-stdlib

gb run
gb check
gb ir
gb build
./build/hello-genix
```

Release build:

```bash
gb build --release
```

## Compiler architecture

```text
Genix application + stdlib modules
        ↓
Project/module loader
        ↓
Lexer → Parser → AST
        ↓
Static Type Checker
        ↓
Typed Genix IR
        ↓
C11 backend
        ↓
Generated application
        +
Genix Runtime ABI v1
        ↓
Native executable
```

The C backend consumes typed Genix IR, not the parser AST.

## Standard library

Official modules are imported with normal Genix syntax:

```gb
import io;
import fs;
import process;
import math;
import string;
```

Current modules include:

- `io` — typed output and `io.input(...)`
- `fs` — `read_text(...)` and `write_text(...)`
- `process` — environment lookup and process exit
- `math` — basic numeric helpers
- `string` — concatenation and equality helpers

Module lookup order is project-local first, then `GENIX_STDLIB/modules/`. The compiler validates `genix-stdlib/COMPATIBILITY` before loading official modules.

## Host-backed standard-library APIs

Genix now has a small **bootstrap native intrinsic boundary** for foundational OS-facing stdlib functionality.

```gb
import io;
import fs;
import process;

fn main() {
    let name: string = io.input("Your name: ");

    fs.write_text("hello.txt", "Hello " + name);
    io.println(fs.read_text("hello.txt"));

    io.println(process.env("HOME"));
}
```

The public APIs are ordinary stdlib functions. Their implementation is selected by the execution path:

```text
Public Genix API     gb run / interpreter       gb build / native
---------------------------------------------------------------------
io.input             Rust stdin                 gb_input
fs.read_text         Rust filesystem            gb_fs_read_text
fs.write_text        Rust filesystem            gb_fs_write_text
process.env          Rust environment           gb_env_get
process.exit         Rust process exit           gb_process_exit
```

This keeps application code portable while avoiding direct C/platform declarations in `.gb` source.

The bootstrap intrinsic mapping is intentionally narrow. A general native FFI remains a separate future feature.

## Runtime integration

Native code targets:

```text
genix-runtime/include/genix/runtime.h
genix-runtime/src/runtime.c
```

Runtime ABI v1 currently provides lifecycle management, tracked allocation, panic handling, strings, typed output, stdin, environment lookup, text-file I/O, and process exit.

Before native compilation, the compiler verifies:

```c
#define GENIX_RUNTIME_ABI_VERSION 1
```

so an incompatible runtime is rejected before invoking `cc`, `clang`, or `gcc`.

## Genix IR

```bash
gb ir
gb ir path/to/project
```

IR carries resolved function names, module-qualified calls, typed expressions and variables, structured control flow, and explicit safe casts such as `int → float`.

## Developer CLI

```text
gb new <name>                  Create a new Genix project
gb run [target]                Execute through the interpreter
gb check [target]              Check syntax, modules, stdlib, and types
gb ir [target]                 Print typed Genix IR
gb build [project] [--release] Build and link a native executable
gb version                     Show the current version
gb help                        Show help
```

## Implemented language/toolchain features

- `.gb` source files and `genix.toml` projects
- Multi-file and official stdlib modules
- Typed functions, parameters, returns, and variables
- `int`, `float`, `string`, `bool`
- Type inference and static checking
- Safe `int → float` widening
- `let` / `mut`
- Arithmetic, comparisons, and boolean logic
- `if` / `else`, `while`, lexical block scope
- Typed Genix IR
- C11 native backend
- Debug and release native builds
- External runtime and standard-library integration
- Host-backed input/filesystem/environment/process APIs
- Cross-repository compiler/runtime/stdlib CI

## Current limitations

- Genix is pre-alpha and not production-ready.
- Native builds currently target the host platform only.
- The bootstrap native backend requires a C compiler.
- Runtime and stdlib currently need to be discoverable locally.
- Runtime string representation and memory ownership are temporary.
- Filesystem errors do not yet use structured `Result` values.
- Missing environment variables currently return an empty string.
- Nested module imports and packages/registry are not implemented yet.
- General native FFI, LLVM, and WebAssembly backends are not implemented yet.

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

## Next milestones

The next priorities are:

- Structured error model with `Result` / `Option`
- Source-span diagnostics
- `gb test`
- `gb fmt`
- Stable toolchain installation/discovery
- General native FFI declarations
- Filesystem/path/time expansion
- Package management and lockfiles
- Ownership/memory-safety design
- Target triples and cross-compilation
- LLVM and WebAssembly backends
- Concurrency / async
- Language server and editor tooling

## Ecosystem

- `genix-lang` — compiler and core language implementation
- `genix-runtime` — runtime ABI and low-level host services
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
| Compiler identity | `gbc` |
| Intermediate representation | Genix IR |
| Runtime | Genix Runtime ABI |
| Standard library | Genix Stdlib |

## Development

The compiler is implemented in **Rust**. The current runtime is portable **C11**.

```bash
export GENIX_RUNTIME=/path/to/genix-runtime
export GENIX_STDLIB=/path/to/genix-stdlib
cargo check
cargo test
cargo run -- run examples/intrinsics
cargo run -- build examples/intrinsics
```

GitHub Actions validates interpreter and native behavior across `genix-lang`, `genix-runtime`, and `genix-stdlib`, including real stdin, filesystem, and environment access.

---

**Genix — by GenixBit**
