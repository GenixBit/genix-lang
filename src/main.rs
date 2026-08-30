mod ast;
mod interpreter;
mod lexer;
mod parser;
mod project;
mod typechecker;

use std::path::Path;
use std::{env, fs, process};

fn main() {
    let mut args = env::args().skip(1);
    let command = args.next();

    let result = match command.as_deref() {
        Some("new") => {
            let Some(path) = args.next() else {
                eprintln!("usage: gb new <project-name>");
                process::exit(2);
            };
            create_project(&path)
        }
        Some("run") => run_target(args.next().as_deref()),
        Some("check") => check_target(args.next().as_deref()),
        Some("build") => build_target(args.next().as_deref()),
        Some("version") | Some("--version") | Some("-V") => {
            println!("Genix 0.0.1 (pre-alpha)");
            Ok(())
        }
        Some("help") | Some("--help") | Some("-h") | None => {
            print_help();
            Ok(())
        }
        Some(other) => {
            eprintln!("error: unknown command '{other}'");
            print_help();
            process::exit(2);
        }
    };

    if let Err(error) = result {
        eprintln!("Genix error: {error}");
        process::exit(1);
    }
}

fn create_project(path: &str) -> Result<(), String> {
    let config = project::create_project(Path::new(path))?;
    println!("✓ created Genix project '{}'", config.name);
    println!("  {path}/genix.toml");
    println!("  {path}/src/main.gb");
    println!();
    println!("Next:");
    println!("  cd {path}");
    println!("  gb run");
    Ok(())
}

fn run_target(target: Option<&str>) -> Result<(), String> {
    if let Some(path) = target.filter(|path| is_gb_file(path)) {
        let source = read_source(path)?;
        return run_source(&source);
    }

    let root = target.unwrap_or(".");
    let loaded = project::load_project(Path::new(root))?;
    interpreter::execute(&loaded.program)
}

fn check_target(target: Option<&str>) -> Result<(), String> {
    if let Some(path) = target.filter(|path| is_gb_file(path)) {
        let source = read_source(path)?;
        check_source(&source)?;
        println!("✓ {path} passed Genix syntax and type checks");
        return Ok(());
    }

    let root = target.unwrap_or(".");
    let loaded = project::load_project(Path::new(root))?;
    println!(
        "✓ project '{}' passed Genix syntax, module, and type checks",
        loaded.config.name
    );
    Ok(())
}

fn build_target(target: Option<&str>) -> Result<(), String> {
    let root = target.unwrap_or(".");
    if is_gb_file(root) {
        return Err("gb build currently operates on Genix projects with genix.toml".into());
    }

    let loaded = project::load_project(Path::new(root))?;
    let artifact = project::write_frontend_artifact(&loaded)?;
    println!("✓ frontend build completed for '{}'", loaded.config.name);
    println!("  {}", artifact.display());
    println!("  native executable generation is the next backend milestone");
    Ok(())
}

fn read_source(path: &str) -> Result<String, String> {
    if !is_gb_file(path) {
        return Err("Genix source files must use the .gb extension".into());
    }
    fs::read_to_string(path).map_err(|error| format!("could not read {path}: {error}"))
}

fn is_gb_file(path: &str) -> bool {
    Path::new(path).extension().and_then(|value| value.to_str()) == Some("gb")
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
    println!("  gb new <name>        Create a new Genix project");
    println!("  gb run [target]      Run a .gb file or project (default: current project)");
    println!("  gb check [target]    Check a .gb file or project");
    println!("  gb build [project]   Produce a checked frontend build artifact");
    println!("  gb version           Show the current version");
    println!("  gb help              Show this help");
    println!();
    println!("Project layout:");
    println!("  genix.toml");
    println!("  src/main.gb");
}
