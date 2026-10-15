use std::path::Path;

use anyhow::{Context, Result, bail};
use crownplugin_model::EntityId;

const PLUGIN_MANIFEST: &str = include_str!("templates/plugin.ron");
const PLUGIN_SCRIPT: &str = include_str!("templates/main.luau");
const THEME_MANIFEST: &str = include_str!("templates/theme.ron");

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Template {
    Plugin,
    Theme,
}

fn fill(template: &str, id: &EntityId, name: &str) -> String {
    template.replace("{{id}}", id.as_str()).replace("{{name}}", name)
}

pub fn create(kind: Template, id: &str, name: Option<&str>, dir: Option<&Path>) -> Result<std::path::PathBuf> {
    let id = EntityId::new(id)?;
    let name = name.map_or_else(|| id.as_str().rsplit('.').next().unwrap_or_default().to_owned(), str::to_owned);
    let dir = dir.map_or_else(|| Path::new(id.as_str()).to_owned(), Path::to_owned);
    if dir.exists() && std::fs::read_dir(&dir)?.next().is_some() {
        bail!("{} already exists and is not empty", dir.display());
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let files: &[(&str, &str)] = match kind {
        Template::Plugin => &[("plugin.ron", PLUGIN_MANIFEST), ("main.luau", PLUGIN_SCRIPT)],
        Template::Theme => &[("theme.ron", THEME_MANIFEST)],
    };
    for (file, template) in files {
        std::fs::write(dir.join(file), fill(template, &id, &name))?;
    }
    Ok(dir)
}
