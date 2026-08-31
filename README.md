# Genix

**Genix** is a modern programming language by **GenixBit**. Genix source files use the **`.gb`** extension.

> Status: **pre-alpha / v0.0.1**. Syntax, runtime behavior, standard-library APIs, and compiler interfaces may change.

## Create, format, run, test, inspect, and compile

```bash
gb new hello-genix
cd hello-genix

export GENIX_RUNTIME=/path/to/genix-runtime
export GENIX_STDLIB=/path/to/genix-stdlib

gb fmt --check
gb check
gb test
gb run
gb ir
gb build
./build/hello-genix
```

`gb new` creates both `src/main.gb` and a starter `tests/smoke.gb` test.

Release build:

```bash
gb build --release
```

## Compiler architecture

```text
Genix application + stdlib modules
        ↓
Project/module loader + SourceMap
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

Testing and formatting are separate developer-tooling paths so they do not pollute application IR/native builds:

```text
tests/**/*.gb                  src/**/*.gb + tests/**/*.gb
     ↓                                      ↓
Genix test frontend                         gb fmt
     ↓                                      ↓
parser + AST + checker          comment-preserving tokenizer
     ↓                                      ↓
fresh interpreter per test        canonical source printer
     ↓
T0001 / T0002 diagnostics
```

## Genix formatting

Genix includes one canonical source formatter:

```bash
gb fmt
gb fmt path/to/project
gb fmt src/main.gb
gb fmt --check
```

For a project target, `gb fmt` recursively formats:

```text
src/**/*.gb
tests/**/*.gb
```

The current canonical style includes:

- four-space block indentation
- one space around binary operators
- `name: type` annotations
- canonical function signatures such as `fn add(a: int, b: int) -> int`
- compact bootstrap generic punctuation such as `Option<string>` and `Result<string,string>`
- consistently expanded blocks and match arms
- blank lines between top-level declarations
- preservation of string literal contents
- preservation of standalone and trailing `//` comments

Example:

```gb
fn add(a:int,b:int)->int{return a+b;}
```

becomes:

```gb
fn add(a: int, b: int) -> int {
    return a + b;
}
```

`gb fmt --check` performs no writes and returns a non-zero process status when formatting changes are required. Formatter output is idempotent, making it suitable for CI.

The formatter is deliberately lexical/comment-preserving instead of AST-based because the executable AST does not retain comments. Genix IR is not involved in formatting.

## Genix testing

Project tests live under `tests/`:

```gb
test "addition works" {
    assert(2 + 2 == 4);
}

test "comparison works" {
    let value: int = 12;
    assert(value > 10);
}
```

Run the suite with:

```bash
gb test
gb test path/to/project
gb test path/to/test.gb
```

Current test helpers are:

```text
assert(condition)   Fail if a bool condition is false
fail(message)       Deliberately fail the test and preserve the message
pass()              Explicit successful no-op
```

Test files may define ordinary helper functions outside test blocks. Each test executes using a fresh interpreter instance, and a suite with any failure returns a non-zero process status.

Successful output:

```text
Genix Test Runner

✓ addition works
✓ comparison works

2 passed
0 failed
```

Failed assertions are now source-aware test diagnostics:

```text
✗ addition works
error[T0001]: assertion failed
 --> tests/math.gb:2:12
   |
 2 |     assert(2 + 2 == 5);
   |            ^^^^^^^^^^ assertion evaluated to false
```

Explicit failures use `T0002` and retain their message:

```text
error[T0002]: test failed: expected Err
```

Genuine interpreter failures remain runtime errors rather than being classified as assertions. The old division-by-zero assertion sentinel has been removed.

The current pre-alpha runner records each assertion/fail source site and uses a private per-run failure trap to carry the site ID/message back to the runner. Internally that trap currently reuses an existing host-read failure path; it is not a public filesystem or Runtime ABI contract and can later be replaced by a structured interpreter-level test signal without changing `T0001` / `T0002` behavior.

The test DSL is recognized by `gb test`, not by the normal application parser path used by `gb run`, `gb check`, `gb ir`, and `gb build`.

## Compiler diagnostics and source maps

Genix has coded, source-aware compiler diagnostics.

```text
error[E0201]: initializer for 'age' expected int, found string
 --> src/main.gb:2:20
   |
 2 |     let age: int = "twenty";
   |                    ^^^^^^^^ type mismatch
  = help: change the expression or annotation so the types are compatible
```

Current compiler error-code families:

```text
E000x  lexer
E010x  parser / syntax
E020x  static type checking
```

Test failures use the separate `T000x` family.

Project/module loading now maintains a frontend `SourceMap` containing original source text plus canonical function/module ownership. Imported-module lexer/parser failures therefore retain their real file names, and semantic failures can be mapped back to the module that supplied the failing function after project functions are merged for checking.

Diagnostics support related locations as secondary labels:

```text
 --> src/math.gb:2:22
 ::: src/main.gb:4:11
```

These can show where a module was referenced or where a related function was defined. Source-map and terminal presentation metadata stay outside executable AST/IR structures.

## Typed error handling

Genix supports primitive-payload `Option` and `Result` values.

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
gb check [target]              Check a .gb file or project
gb test [target]               Run tests/*.gb or a standalone test file
gb fmt [target] [--check]      Format Genix source or verify canonical formatting
gb ir [target]                 Print typed Genix IR
gb build [project] [--release] Build and link a native executable
gb version                     Show the current version
gb help                        Show help
```

## Implemented language/toolchain features

- `.gb` source files and `genix.toml` projects
- Multi-file and official stdlib modules
- Multi-file `SourceMap` with function/module ownership
- Primary and secondary diagnostic locations
- First-class `gb fmt` canonical formatter
- Recursive `src/**/*.gb` and `tests/**/*.gb` formatting
- Comment- and string-preserving lexical formatting
- Formatter idempotence and CI-ready `gb fmt --check`
- First-class `gb test` developer workflow
- Recursive `tests/**/*.gb` discovery
- Named `test "..." { ... }` blocks
- `assert`, `fail`, and `pass` testing helpers
- Dedicated `T0001` / `T0002` test diagnostics
- Exact assertion/fail source spans and `fail(message)` reporting
- Runtime errors distinct from assertion/test failures
- Isolated interpreter instance per test
- Non-zero test-suite exit status on failure
- Typed functions, parameters, returns, and variables
- `int`, `float`, `string`, `bool`
- Primitive-payload `Option<T>` and `Result<T,string>`
- `Some`, `None`, `Ok`, `Err`
- Exhaustive `match` for Option/Result
- Result propagation with `?`
- Type inference and static checking
- Coded source-aware lexer/parser/type diagnostics
- Error spans, labels, help text, and stable error-code families
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
- Semantic source spans are currently resolved by the frontend/project diagnostics adapter; the checker does not yet return structured source diagnostics directly.
- The formatter does not yet wrap long lines, reorder imports, format `genix.toml`, or expose style configuration.
- Test declarations are currently handled only by the `gb test` frontend.
- Test-file imports are not independently resolved yet.
- Runtime errors in tests currently report the enclosing test-block location instead of the exact runtime expression.
- Test assertion value diffs are not implemented yet.
- The private test-failure trap currently piggybacks on an existing host-read failure path and should become a structured interpreter-level signal later.
- Nested module imports and packages/registry are not implemented yet.
- General native FFI, LLVM, and WebAssembly backends are not implemented yet.

## Repository architecture

```text
src/
├── ast.rs
├── diagnostics.rs
├── source_map.rs
├── formatter.rs
├── lexer.rs
├── parser.rs
├── typechecker.rs
├── testing.rs
├── ir.rs
├── interpreter.rs
├── project.rs
├── codegen.rs
└── main.rs
```

## Next milestones

The next priorities are:

- Checker-native structured semantic diagnostics with source IDs/spans
- Machine-readable diagnostics groundwork for LSP/editor integrations
- Generalized enums and generics
- Expected/actual assertion value reporting and test filtering
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
cargo run -- fmt /tmp/genix-project --check
cargo run -- test /tmp/genix-project
cargo run -- run examples/error_handling
cargo run -- ir examples/error_handling
cargo run -- build examples/error_handling
./examples/error_handling/build/error-handling-demo
```

GitHub Actions validates interpreter and native behavior across `genix-lang`, `genix-runtime`, and `genix-stdlib`. It verifies formatter behavior, project and standalone testing, source-aware diagnostics, runtime/stdlib integration, safe error handling, and debug/release native builds. Test-runner unit coverage specifically verifies that assertion/fail trap signals decode separately from genuine division-by-zero runtime failures.

---

**Genix — by GenixBit**
