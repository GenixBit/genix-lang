mod ast;
mod codegen;
mod diagnostics;
mod formatter;
mod interpreter;
mod ir;
mod lexer;
mod parser;
mod project;
mod source_map;
mod testing;
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
        Some("test") => test_target(args.next().as_deref()),
        Some("fmt") => fmt_target(args.collect()),
        Some("ir") => ir_target(args.next().as_deref()),
        Some("build") => build_target(args.collect()),
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
        if error.starts_with("error[") {
            eprint!("{error}");
            if !error.ends_with('\n') {
                eprintln!();
            }
        } else {
            eprintln!("Genix error: {error}");
        }
        process::exit(1);
    }
}

fn create_project(path: &str) -> Result<(), String> {
    let config = project::create_project(Path::new(path))?;
    let tests_dir = Path::new(path).join("tests");
    fs::create_dir_all(&tests_dir)
        .map_err(|error| format!("could not create tests directory: {error}"))?;
    fs::write(
        tests_dir.join("smoke.gb"),
        "test \"arithmetic works\" {\n    assert(2 + 2 == 4);\n}\n",
    )
    .map_err(|error| format!("could not write tests/smoke.gb: {error}"))?;

    println!("✓ created Genix project '{}'", config.name);
    println!("  {path}/genix.toml");
    println!("  {path}/src/main.gb");
    println!("  {path}/tests/smoke.gb");
    println!();
    println!("Next:");
    println!("  cd {path}");
    println!("  gb run");
    println!("  gb test");
    Ok(())
}

fn run_target(target: Option<&str>) -> Result<(), String> {
    if let Some(path) = target.filter(|path| is_gb_file(path)) {
        let source = read_source(path)?;
        return run_source(path, &source);
    }

    let root = target.unwrap_or(".");
    let loaded = project::load_project(Path::new(root))?;
    interpreter::execute(&loaded.program)
}

fn check_target(target: Option<&str>) -> Result<(), String> {
    if let Some(path) = target.filter(|path| is_gb_file(path)) {
        let source = read_source(path)?;
        check_source(path, &source)?;
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

fn test_target(target: Option<&str>) -> Result<(), String> {
    testing::run(Path::new(target.unwrap_or(".")))
}

fn fmt_target(args: Vec<String>) -> Result<(), String> {
    let mut target: Option<String> = None;
    let mut check = false;

    for arg in args {
        if arg == "--check" {
            check = true;
        } else if arg.starts_with('-') {
            return Err(format!("unknown fmt option '{arg}'"));
        } else if target.is_none() {
            target = Some(arg);
        } else {
            return Err("gb fmt accepts at most one target path".into());
        }
    }

    let target = Path::new(target.as_deref().unwrap_or("."));
    let summary = formatter::format_target(target, check)?;

    if check {
        if summary.changed.is_empty() {
            println!("✓ {} Genix file(s) are canonically formatted", summary.files);
            return Ok(());
        }

        println!("Genix formatting check failed:");
        for path in &summary.changed {
            println!("  {}", path.display());
        }
        return Err(format!(
            "{} Genix file(s) need formatting; run 'gb fmt' to update them",
            summary.changed.len()
        ));
    }

    if summary.changed.is_empty() {
        println!("✓ {} Genix file(s) already formatted", summary.files);
    } else {
        for path in &summary.changed {
            println!("✓ formatted {}", path.display());
        }
        println!("{} Genix file(s) formatted", summary.changed.len());
    }
    Ok(())
}

fn ir_target(target: Option<&str>) -> Result<(), String> {
    if let Some(path) = target.filter(|path| is_gb_file(path)) {
        let source = read_source(path)?;
        let ast = compile_frontend(path, &source)?;
        let lowered = ir::lower(&ast)?;
        print!("{}", ir::format(&lowered));
        return Ok(());
    }

    let root = target.unwrap_or(".");
    let loaded = project::load_project(Path::new(root))?;
    let lowered = ir::lower(&loaded.program)?;
    print!("{}", ir::format(&lowered));
    Ok(())
}

fn build_target(args: Vec<String>) -> Result<(), String> {
    let mut target: Option<String> = None;
    let mut release = false;

    for arg in args {
        if arg == "--release" {
            release = true;
        } else if arg.starts_with('-') {
            return Err(format!("unknown build option '{arg}'"));
        } else if target.is_none() {
            target = Some(arg);
        } else {
            return Err("gb build accepts at most one project path".into());
        }
    }

    let root = target.as_deref().unwrap_or(".");
    if is_gb_file(root) {
        return Err("gb build operates on Genix projects with genix.toml".into());
    }

    let loaded = project::load_project(Path::new(root))?;
    let lowered = ir::lower(&loaded.program)?;
    let artifact = codegen::build_native(
        &lowered,
        &loaded.root,
        &loaded.config.name,
        release,
    )?;

    let profile = if artifact.release { "release" } else { "debug" };
    println!(
        "✓ native {profile} build completed for '{}'",
        loaded.config.name
    );
    println!("  pipeline: AST -> typed Genix IR -> C11 -> Genix Runtime -> native executable");
    println!("  runtime: {}", artifact.runtime_root.display());
    println!("  compiler: {}", artifact.compiler);
    println!("  C source: {}", artifact.source.display());
    println!("  executable: {}", artifact.executable.display());
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

fn compile_frontend(source_name: &str, source: &str) -> Result<ast::Program, String> {
    let tokens = lexer::lex_diagnostic(source).map_err(|diagnostic| {
        diagnostic
            .with_source_name(source_name.to_string())
            .render(Some(source))
    })?;
    let program = parser::parse_named(tokens, source_name)
        .map_err(|diagnostic| diagnostic.render(Some(source)))?;

    let mut source_map = source_map::SourceMap::new();
    source_map.add_file(source_name.to_string(), source.to_string());
    source_map.set_entry(source_name.to_string());
    for function in &program.functions {
        source_map.bind_function(function.name.clone(), source_name.to_string());
    }

    typechecker::check_diagnostic(&program, &source_map)
        .map_err(|diagnostic| diagnostic.render(Some(source)))?;
    Ok(program)
}

fn check_source(source_name: &str, source: &str) -> Result<(), String> {
    compile_frontend(source_name, source)?;
    Ok(())
}

fn run_source(source_name: &str, source: &str) -> Result<(), String> {
    let program = compile_frontend(source_name, source)?;
    interpreter::execute(&program)
}

fn print_help() {
    println!("Genix developer CLI");
    println!();
    println!("Usage:");
    println!("  gb new <name>                  Create a new Genix project");
    println!("  gb run [target]                Run a .gb file or project");
    println!("  gb check [target]              Check a .gb file or project");
    println!("  gb test [target]               Run tests/*.gb or a standalone test file");
    println!("  gb fmt [target] [--check]      Format Genix source or verify canonical formatting");
    println!("  gb ir [target]                 Print typed Genix intermediate representation");
    println!("  gb build [project] [--release] Build a native executable from Genix IR");
    println!("  gb version                     Show the current version");
    println!("  gb help                        Show this help");
    println!();
    println!("Test syntax:");
    println!("  test \"addition works\" {{");
    println!("      assert(2 + 2 == 4);");
    println!("  }}");
    println!();
    println!("Formatter:");
    println!("  gb fmt                         Format src/**/*.gb and tests/**/*.gb");
    println!("  gb fmt src/main.gb             Format one source file");
    println!("  gb fmt --check                 Fail if project files need formatting");
    println!();
    println!("Native build requirements:");
    println!("  cc, clang, or gcc on PATH (or set CC)");
    println!("  Genix Runtime available through GENIX_RUNTIME or a discoverable genix-runtime directory");
    println!();
    println!("Example:");
    println!("  export GENIX_RUNTIME=/path/to/genix-runtime");
    println!("  gb build --release");
    println!();
    println!("Project layout:");
    println!("  genix.toml");
    println!("  src/main.gb");
    println!("  tests/*.gb");
}
