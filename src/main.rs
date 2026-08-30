mod ast;
mod interpreter;
mod lexer;
mod parser;
mod typechecker;

use std::{env, fs, process};

fn main() {
    let mut args = env::args().skip(1);
    let command = args.next();

    match command.as_deref() {
        Some("run") => {
            let Some(path) = args.next() else {
                eprintln!("usage: gb run <file.gb>");
                process::exit(2);
            };

            if !path.ends_with(".gb") {
                eprintln!("error: Genix source files must use the .gb extension");
                process::exit(2);
            }

            let source = match fs::read_to_string(&path) {
                Ok(source) => source,
                Err(error) => {
                    eprintln!("error: could not read {path}: {error}");
                    process::exit(1);
                }
            };

            if let Err(error) = run_source(&source) {
                eprintln!("Genix error: {error}");
                process::exit(1);
            }
        }
        Some("check") => {
            let Some(path) = args.next() else {
                eprintln!("usage: gb check <file.gb>");
                process::exit(2);
            };

            if !path.ends_with(".gb") {
                eprintln!("error: Genix source files must use the .gb extension");
                process::exit(2);
            }

            let source = match fs::read_to_string(&path) {
                Ok(source) => source,
                Err(error) => {
                    eprintln!("error: could not read {path}: {error}");
                    process::exit(1);
                }
            };

            match check_source(&source) {
                Ok(()) => println!("✓ {path} passed Genix syntax and type checks"),
                Err(error) => {
                    eprintln!("Genix error: {error}");
                    process::exit(1);
                }
            }
        }
        Some("version") | Some("--version") | Some("-V") => {
            println!("Genix 0.0.1 (pre-alpha)");
        }
        Some("help") | Some("--help") | Some("-h") | None => print_help(),
        Some(other) => {
            eprintln!("error: unknown command '{other}'");
            print_help();
            process::exit(2);
        }
    }
}

fn compile_frontend(source: &str) -> Result<ast::Program, String> {
    let tokens = lexer::lex(source)?;
    let program = parser::parse(tokens)?;
    typechecker::check(&program)?;
    Ok(program)
}

fn check_source(source: &str) -> Result<(), String> {
    compile_frontend(source)?;
    Ok(())
}

fn run_source(source: &str) -> Result<(), String> {
    let program = compile_frontend(source)?;
    interpreter::execute(&program)
}

fn print_help() {
    println!("Genix developer CLI");
    println!();
    println!("Usage:");
    println!("  gb run <file.gb>     Type-check and execute a Genix source file");
    println!("  gb check <file.gb>   Validate Genix syntax and types");
    println!("  gb version           Show the current version");
    println!("  gb help              Show this help");
}
