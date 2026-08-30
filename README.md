# Genix

**Genix** is a modern programming language by **GenixBit**, designed for expressive application development, AI-native software, backend services, automation, and high-performance tooling.

Genix source files use the **`.gb`** extension.

> Status: early development / pre-alpha. The syntax and APIs are not yet stable.

## Vision

Genix aims to combine a clean developer experience with strong typing, memory safety, modern concurrency, native performance, and first-class support for AI workloads.

```gb
fn main() {
    print("Hello from Genix!")
}
```

Planned CLI experience:

```bash
gb run hello.gb
gb build
gb test
gb fmt
gb check
```

## Repository scope

This repository is the flagship implementation of the Genix language and will contain:

- Lexer and tokenizer
- Parser and abstract syntax tree (AST)
- Semantic analysis
- Type system
- Intermediate representations
- Diagnostics
- Compiler frontend and code generation
- `gb` developer CLI during the early implementation phase
- Compiler tests and language conformance tests

## Initial language goals

The first developer milestone focuses on a deliberately small core:

- `.gb` source files
- `fn main()` entry point
- Variables and constants
- Strings, integers, floats, and booleans
- Arithmetic and comparison operators
- `if` / `else`
- Loops
- Functions
- Static type checking
- Imports/modules
- `print()`
- Useful compiler diagnostics

Advanced features such as AI primitives, async/concurrency, packages, web frameworks, and native code generation will be introduced incrementally after the core is stable.

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
| Compiler | `gbc` |

## Development

The initial compiler toolchain is being implemented in Rust. Architecture and contribution documentation will evolve alongside the first executable language prototype.

## Project status

Genix is experimental and not yet suitable for production use. Breaking language and compiler changes should be expected before the first stable release.

---

**Genix — by GenixBit**
