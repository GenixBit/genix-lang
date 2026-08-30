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

## Typed error handling

Genix now supports primitive-payload `Option` and `Result` values.

```gb
fn load(path: string) -> Result<string,string> {
    let text: string = fs.try_read_text(path)?;
    return Ok(text);
}
```

Available forms currently include:

```text
Option<int>
Option<float>
Option<bool>
Option<string>

Result<int,string>
Result<float,string>
Result<bool,string>
Result<string,string>
```

Constructors:

```text
Some(value)
None
Ok(value)
Err(error)
```

`match` is exhaustive for `Option` and `Result`:

```gb
match result {
    Ok(value) => {
        print(value);
    }
    Err(error) => {
        print(error);
    }
}
```

`?` propagates `Err(error)` from functions returning `Result<...,string>`.

The bootstrap implementation currently allows `?` when it is the complete value of a variable initializer, assignment, or call expression statement. Arbitrary nested generic types, custom Result error types, and user-defined enums are future generalizations.

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
- `fs` — text I/O plus recoverable `try_read_text` / `try_write_text`
- `process` — environment lookup, optional lookup, and process exit
- `math` — basic numeric helpers
- `string` — concatenation and equality helpers

Module lookup order is project-local first, then `GENIX_STDLIB/modules/`. The compiler validates `genix-stdlib/COMPATIBILITY` before loading official modules.

## Safe host-backed APIs

Preferred recoverable/optional APIs:

```gb
import fs;
import process;

fn save_and_load(path: string) -> Result<string,string> {
    let written: bool = fs.try_write_text(path, "Genix safe IO")?;
    let text: string = fs.try_read_text(path)?;
    return Ok(text);
}

fn main() {
    let home: Option<string> = process.env_option("HOME");

    match home {
        Some(value) => {
            print(value);
        }
        None => {
            print("HOME is not set");
        }
    }
}
```

Execution mapping:

```text
Public Genix API        gb run / interpreter       gb build / native
--------------------------------------------------------------------------
io.input                Rust stdin                 gb_input
process.env_option      Rust environment           gb_env_get_option
fs.try_read_text        Rust filesystem            gb_fs_try_read_text
fs.try_write_text       Rust filesystem            gb_fs_try_write_text
process.exit            Rust process exit          gb_process_exit
```

Legacy bootstrap `fs.read_text`, `fs.write_text`, and `process.env` remain temporarily available for compatibility.

## Runtime integration

Native code targets:

```text
genix-runtime/include/genix/runtime.h
genix-runtime/src/runtime.c
```

Runtime ABI v1 provides lifecycle management, tracked allocation, panic handling, strings, typed output, stdin, filesystem/environment/process host services, and tagged primitive `Option` / `Result` carrier structures.

Before native compilation, the compiler verifies:

```c
#define GENIX_RUNTIME_ABI_VERSION 1
```

## Genix IR

```bash
gb ir
gb ir path/to/project
```

IR carries resolved function names, module-qualified calls, typed expressions and variables, structured control flow, explicit safe casts, `Some`/`None`, `Ok`/`Err`, exhaustive `match`, and Result propagation.

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
- Primitive-payload `Option<T>` and `Result<T,string>`
- `Some`, `None`, `Ok`, `Err`
- Exhaustive `match` for Option/Result
- Result propagation with `?`
- Type inference and static checking
- Safe `int → float` widening
- `let` / `mut`
- Arithmetic, comparisons, and boolean logic
- `if` / `else`, `while`, lexical block scope
- Typed Genix IR
- C11 native backend
- Debug and release native builds
- External runtime and standard-library integration
- Safe filesystem/environment host APIs
- Cross-repository compiler/runtime/stdlib CI

## Current limitations

- Genix is pre-alpha and not production-ready.
- Native builds currently target the host platform only.
- The bootstrap native backend requires a C compiler.
- Runtime and stdlib currently need to be discoverable locally.
- Runtime string representation and memory ownership are temporary.
- Option/Result currently support primitive payloads only; Result errors are strings.
- `?` placement is intentionally restricted while IR control-flow lowering is generalized.
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

- Source-span diagnostics and substantially better compiler errors
- `gb test`
- `gb fmt`
- Generalized enums and generics
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
cargo run -- run examples/error_handling
cargo run -- ir examples/error_handling
cargo run -- build examples/error_handling
./examples/error_handling/build/error-handling-demo
```

GitHub Actions validates interpreter and native behavior across `genix-lang`, `genix-runtime`, and `genix-stdlib`, including typed error handling and recoverable host I/O.

---

**Genix — by GenixBit**
