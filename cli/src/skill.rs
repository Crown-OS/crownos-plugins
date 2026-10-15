//! Installing the authoring skill for coding agents.

use std::path::PathBuf;

use anyhow::{Context, Result};

const SKILL: &str = include_str!("../../skill/SKILL.md");
const TYPES: &str = include_str!("../../types/crown.d.luau");
const NAME: &str = "crownos-plugins";

/// Every directory a coding agent looks for user skills in.
fn roots() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    vec![home.join(".claude/skills"), home.join(".agents/skills")]
}

pub fn install() -> Result<Vec<PathBuf>> {
    roots()
        .into_iter()
        .map(|root| {
            let dir = root.join(NAME);
            std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
            std::fs::write(dir.join("SKILL.md"), SKILL)?;
            std::fs::write(dir.join("crown.d.luau"), TYPES)?;
            Ok(dir)
        })
        .collect()
}

pub fn types() -> &'static str {
    TYPES
}
