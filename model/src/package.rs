//! Where themes and plugins live on disk.

use std::path::{Component, Path, PathBuf};

use crate::{EntityId, Kind, Manifest, ModelError};

/// Overrides the user data root, for tests and nested sessions.
pub const DATA_DIR_ENV: &str = "CROWN_DATA_DIR";
/// Overrides the state root holding journals and backups.
pub const STATE_DIR_ENV: &str = "CROWN_STATE_DIR";
pub const SYSTEM_ROOT: &str = "/usr/share/crownos";
/// Where a theme's compositor-visible files are installed.
pub const SYSTEM_THEME_ASSETS: &str = "/usr/share/crownos/theme-assets";

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}

/// `~/.local/share/crownos`.
pub fn data_root() -> PathBuf {
    env_dir(DATA_DIR_ENV).unwrap_or_else(|| {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("crownos")
    })
}

/// `~/.local/state/crownos/plugins`.
pub fn state_root() -> PathBuf {
    env_dir(STATE_DIR_ENV).unwrap_or_else(|| {
        dirs::state_dir()
            .or_else(dirs::data_dir)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("crownos")
            .join("plugins")
    })
}

pub fn user_dir(kind: Kind) -> PathBuf {
    data_root().join(kind.dir_name())
}

pub fn system_dir(kind: Kind) -> PathBuf {
    Path::new(SYSTEM_ROOT).join(kind.dir_name())
}

pub fn install_dir(kind: Kind, id: &EntityId) -> PathBuf {
    user_dir(kind).join(id.as_str())
}

/// `rel` as a path inside a package, or `None` if it would leave it.
pub fn relative_path(rel: &str) -> Option<PathBuf> {
    let path = Path::new(rel);
    path.components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
        .then(|| path.to_owned())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    pub dir: PathBuf,
    pub manifest: Manifest,
    /// Shipped under `/usr/share`: cannot be uninstalled by the user.
    pub system: bool,
}

impl Package {
    pub fn load(dir: &Path) -> Result<Self, ModelError> {
        Ok(Self {
            manifest: Manifest::load(dir)?,
            system: dir.starts_with(SYSTEM_ROOT),
            dir: dir.to_owned(),
        })
    }

    pub fn id(&self) -> &EntityId {
        self.manifest.id()
    }

    /// A package-relative path, refusing anything that escapes the package.
    pub fn resolve(&self, rel: &str) -> Option<PathBuf> {
        relative_path(rel).map(|rel| self.dir.join(rel))
    }

    pub fn read_text(&self, rel: &str) -> Result<String, ModelError> {
        let path = self
            .resolve(rel)
            .ok_or_else(|| ModelError::UnsafeArchivePath(rel.to_owned()))?;
        std::fs::read_to_string(&path).map_err(ModelError::io(path))
    }
}

/// Every package under the user and system directories for `kind`; the user
/// copy wins when both carry the same id. Unreadable packages are returned as
/// errors next to the good ones.
pub fn scan(kind: Kind) -> Vec<Result<Package, ModelError>> {
    let mut found: Vec<Result<Package, ModelError>> = Vec::new();
    for root in [user_dir(kind), system_dir(kind)] {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let package = Package::load(&dir);
            let duplicate = package.as_ref().is_ok_and(|package| {
                found
                    .iter()
                    .flatten()
                    .any(|existing| existing.id() == package.id())
            });
            if !duplicate {
                found.push(package);
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_cannot_escape_the_package() {
        assert!(relative_path("wall/night.jpg").is_some());
        assert!(relative_path("./main.luau").is_some());
        assert!(relative_path("../secret").is_none());
        assert!(relative_path("/etc/passwd").is_none());
        assert!(relative_path("a/../../b").is_none());
    }
}
