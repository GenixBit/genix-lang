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

            println!("Genix pre-alpha runner");
            println!("loaded {path} ({} bytes)", source.len());
            println!("compiler pipeline is under development");
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

fn print_help() {
    println!("Genix developer CLI");
    println!();
    println!("Usage:");
    println!("  gb run <file.gb>   Load a Genix source file");
    println!("  gb version         Show the current version");
    println!("  gb help            Show this help");
}
