use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::ast::{Expr, Function, Program, Stmt};
use crate::{lexer, parser, typechecker};

#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub entry: String,
}

pub struct LoadedProject {
    pub root: PathBuf,
    pub config: ProjectConfig,
    pub program: Program,
    pub imports: Vec<String>,
}

pub fn create_project(path: &Path) -> Result<ProjectConfig, String> {
    if path.exists() {
        return Err(format!("cannot create project: '{}' already exists", path.display()));
    }

    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "project path must end with a valid project name".to_string())?;

    validate_project_name(name)?;
    fs::create_dir_all(path.join("src"))
        .map_err(|error| format!("could not create project directory: {error}"))?;

    let config = ProjectConfig {
        name: name.to_string(),
        version: "0.1.0".to_string(),
        entry: "src/main.gb".to_string(),
    };

    let manifest = format!(
        "[project]\nname = \"{}\"\nversion = \"{}\"\nentry = \"{}\"\n",
        config.name, config.version, config.entry
    );
    fs::write(path.join("genix.toml"), manifest)
        .map_err(|error| format!("could not write genix.toml: {error}"))?;

    let main_source = format!(
        "fn main() {{\n    print(\"Hello from {}!\");\n}}\n",
        config.name
    );
    fs::write(path.join("src/main.gb"), main_source)
        .map_err(|error| format!("could not write src/main.gb: {error}"))?;

    Ok(config)
}

pub fn load_project(target: &Path) -> Result<LoadedProject, String> {
    let root = normalize_project_root(target)?;
    let manifest_path = root.join("genix.toml");
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("could not read {}: {error}", manifest_path.display()))?;
    let config = parse_manifest(&manifest)?;
    validate_entry_path(&config.entry)?;

    let entry_path = root.join(&config.entry);
    let entry_source = fs::read_to_string(&entry_path)
        .map_err(|error| format!("could not read project entry {}: {error}", entry_path.display()))?;
    let (imports, stripped_entry) = extract_imports(&entry_source)?;

    let mut program = parse_source(&stripped_entry)
        .map_err(|error| format!("{}: {error}", entry_path.display()))?;

    let source_dir = entry_path.parent().unwrap_or(&root);
    let mut seen = HashSet::new();
    let mut module_functions = Vec::new();

    for module in &imports {
        if !seen.insert(module.clone()) {
            return Err(format!("module '{module}' is imported more than once"));
        }
        validate_module_name(module)?;

        let module_path = source_dir.join(format!("{module}.gb"));
        let module_source = fs::read_to_string(&module_path)
            .map_err(|error| format!("could not read module {}: {error}", module_path.display()))?;
        let (nested_imports, stripped_module) = extract_imports(&module_source)?;
        if !nested_imports.is_empty() {
            return Err(format!(
                "module '{}' contains imports; nested module imports are planned for the next module-system revision",
                module
            ));
        }

        let mut module_program = parse_source(&stripped_module)
            .map_err(|error| format!("{}: {error}", module_path.display()))?;
        namespace_module(module, &mut module_program)?;
        module_functions.extend(module_program.functions);
    }

    module_functions.extend(program.functions);
    program.functions = module_functions;
    typechecker::check(&program)?;

    Ok(LoadedProject {
        root,
        config,
        program,
        imports,
    })
}

pub fn write_frontend_artifact(project: &LoadedProject) -> Result<PathBuf, String> {
    let build_dir = project.root.join("build");
    fs::create_dir_all(&build_dir)
        .map_err(|error| format!("could not create build directory: {error}"))?;

    let artifact_path = build_dir.join("genix.frontend");
    let mut function_names: Vec<&str> = project
        .program
        .functions
        .iter()
        .map(|function| function.name.as_str())
        .collect();
    function_names.sort_unstable();

    let artifact = format!(
        "Genix frontend artifact\nproject={}\nversion={}\nentry={}\nmodules={}\nfunctions={}\ntypecheck=passed\nnative_backend=pending\n",
        project.config.name,
        project.config.version,
        project.config.entry,
        project.imports.join(","),
        function_names.join(",")
    );

    fs::write(&artifact_path, artifact)
        .map_err(|error| format!("could not write frontend build artifact: {error}"))?;
    Ok(artifact_path)
}

fn normalize_project_root(target: &Path) -> Result<PathBuf, String> {
    let root = if target.file_name().and_then(|value| value.to_str()) == Some("genix.toml") {
        target.parent().unwrap_or_else(|| Path::new("."))
    } else {
        target
    };

    if !root.is_dir() {
        return Err(format!("project path '{}' is not a directory", root.display()));
    }
    if !root.join("genix.toml").is_file() {
        return Err(format!(
            "no genix.toml found in '{}'; run 'gb new <name>' to create a project",
            root.display()
        ));
    }
    Ok(root.to_path_buf())
}

fn parse_manifest(source: &str) -> Result<ProjectConfig, String> {
    let mut in_project = false;
    let mut name = None;
    let mut version = None;
    let mut entry = None;

    for (index, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_project = line == "[project]";
            continue;
        }
        if !in_project {
            continue;
        }

        let Some((key, raw_value)) = line.split_once('=') else {
            return Err(format!("invalid genix.toml line {}", index + 1));
        };
        let key = key.trim();
        let value = parse_manifest_string(raw_value.trim(), index + 1)?;
        match key {
            "name" => name = Some(value),
            "version" => version = Some(value),
            "entry" => entry = Some(value),
            _ => {}
        }
    }

    let name = name.ok_or_else(|| "genix.toml [project] requires name".to_string())?;
    validate_project_name(&name)?;
    Ok(ProjectConfig {
        name,
        version: version.unwrap_or_else(|| "0.1.0".to_string()),
        entry: entry.unwrap_or_else(|| "src/main.gb".to_string()),
    })
}

fn parse_manifest_string(value: &str, line: usize) -> Result<String, String> {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        Ok(value[1..value.len() - 1].to_string())
    } else {
        Err(format!("genix.toml line {line}: values must be quoted strings"))
    }
}

fn validate_entry_path(entry: &str) -> Result<(), String> {
    let path = Path::new(entry);
    if path.is_absolute() || path.components().any(|part| matches!(part, Component::ParentDir)) {
        return Err("genix.toml entry must stay inside the project directory".into());
    }
    if path.extension().and_then(|value| value.to_str()) != Some("gb") {
        return Err("genix.toml entry must point to a .gb source file".into());
    }
    Ok(())
}

fn validate_project_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err("project name may contain only letters, numbers, '-' and '_'".into());
    }
    Ok(())
}

fn validate_module_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err("import requires a module name".into());
    };
    if !(first.is_ascii_alphabetic() || first == '_')
        || !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Err(format!("invalid module name '{name}'"));
    }
    Ok(())
}

fn extract_imports(source: &str) -> Result<(Vec<String>, String), String> {
    let mut imports = Vec::new();
    let mut stripped = String::new();

    for (index, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();
        if let Some(rest) = line.strip_prefix("import ") {
            let module = rest.trim_end_matches(';').trim();
            if module.is_empty() || rest.trim() == module && !line.ends_with(';') {
                return Err(format!("invalid import at line {}: expected 'import module;'", index + 1));
            }
            validate_module_name(module)?;
            imports.push(module.to_string());
            stripped.push('\n');
        } else {
            stripped.push_str(raw_line);
            stripped.push('\n');
        }
    }

    Ok((imports, stripped))
}

fn parse_source(source: &str) -> Result<Program, String> {
    parser::parse(lexer::lex(source)?)
}

fn namespace_module(module: &str, program: &mut Program) -> Result<(), String> {
    if program.functions.iter().any(|function| function.name == "main") {
        return Err(format!("module '{module}' cannot define fn main()"));
    }

    let local_names: HashSet<String> = program
        .functions
        .iter()
        .map(|function| function.name.clone())
        .collect();

    for function in &mut program.functions {
        rewrite_statements(&mut function.body, module, &local_names);
        function.name = format!("{module}.{}", function.name);
    }
    Ok(())
}

fn rewrite_statements(statements: &mut [Stmt], module: &str, local_names: &HashSet<String>) {
    for statement in statements {
        match statement {
            Stmt::Let { value, .. } | Stmt::Assign { value, .. } | Stmt::Print(value) | Stmt::Expr(value) => {
                rewrite_expr(value, module, local_names);
            }
            Stmt::Return(Some(value)) => rewrite_expr(value, module, local_names),
            Stmt::Return(None) => {}
            Stmt::If { condition, then_branch, else_branch } => {
                rewrite_expr(condition, module, local_names);
                rewrite_statements(then_branch, module, local_names);
                if let Some(branch) = else_branch {
                    rewrite_statements(branch, module, local_names);
                }
            }
            Stmt::While { condition, body } => {
                rewrite_expr(condition, module, local_names);
                rewrite_statements(body, module, local_names);
            }
            Stmt::Block(body) => rewrite_statements(body, module, local_names),
        }
    }
}

fn rewrite_expr(expr: &mut Expr, module: &str, local_names: &HashSet<String>) {
    match expr {
        Expr::Call { callee, arguments } => {
            if local_names.contains(callee) {
                *callee = format!("{module}.{callee}");
            }
            for argument in arguments {
                rewrite_expr(argument, module, local_names);
            }
        }
        Expr::Unary { expr, .. } => rewrite_expr(expr, module, local_names),
        Expr::Binary { left, right, .. } => {
            rewrite_expr(left, module, local_names);
            rewrite_expr(right, module, local_names);
        }
        Expr::Integer(_) | Expr::Float(_) | Expr::Bool(_) | Expr::String(_) | Expr::Variable(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_project_manifest() {
        let config = parse_manifest("[project]\nname = \"demo\"\nversion = \"1.2.3\"\nentry = \"src/main.gb\"\n").unwrap();
        assert_eq!(config.name, "demo");
        assert_eq!(config.version, "1.2.3");
        assert_eq!(config.entry, "src/main.gb");
    }

    #[test]
    fn extracts_imports_and_preserves_source() {
        let (imports, source) = extract_imports("import math;\nfn main() { print(math.add(1, 2)); }\n").unwrap();
        assert_eq!(imports, vec!["math"]);
        assert!(source.contains("fn main()"));
    }

    #[test]
    fn namespaces_internal_module_calls() {
        let mut program = parse_source("fn add(a: int, b: int) -> int { return a + b; } fn twice(x: int) -> int { return add(x, x); }").unwrap();
        namespace_module("math", &mut program).unwrap();
        assert_eq!(program.functions[0].name, "math.add");
        assert_eq!(program.functions[1].name, "math.twice");
        match &program.functions[1].body[0] {
            Stmt::Return(Some(Expr::Call { callee, .. })) => assert_eq!(callee, "math.add"),
            _ => panic!("expected rewritten module call"),
        }
    }
}
