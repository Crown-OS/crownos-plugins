//! `crownplugin validate`: manifest, files and Luau syntax, reported as
//! `path: message` lines a person or an AI can act on directly.

use std::path::Path;

use anyhow::Result;
use crownplugin_model::{Package, validate};

pub struct Findings {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

fn luau_files(dir: &Path, found: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            luau_files(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "luau") {
            found.push(path);
        }
    }
}

pub fn check(dir: &Path) -> Result<Findings> {
    let package = match Package::load(dir) {
        Ok(package) => package,
        Err(error) => {
            return Ok(Findings { errors: vec![error.to_string()], warnings: Vec::new() });
        }
    };
    let report = validate(&package);
    let mut errors: Vec<String> = report.errors.iter().map(ToString::to_string).collect();
    let warnings = report.warnings.iter().map(ToString::to_string).collect();
    let mut scripts = Vec::new();
    luau_files(dir, &mut scripts);
    let lua = mlua::Lua::new();
    for script in scripts {
        let source = std::fs::read_to_string(&script)?;
        let name = script.strip_prefix(dir).unwrap_or(&script).display().to_string();
        if let Err(error) = lua.load(source).set_name(format!("@{name}")).into_function() {
            errors.push(error.to_string());
        }
    }
    Ok(Findings { errors, warnings })
}
