//! Write-through with a way back.
//!
//! Every change a theme or plugin makes to the user's config or files is
//! recorded here *before* it is written, with the value it replaced. Turning
//! the entity off walks the record backwards and restores a value only if it
//! still holds what the entity wrote, so anything the user changed in the
//! meantime is kept.

mod apply;
mod restore;

use std::io;
use std::path::PathBuf;

use crownplugin_model::package::state_root;
use serde::{Deserialize, Serialize};

pub use apply::{Change, apply};
pub use restore::restore;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    Set {
        section: String,
        key: String,
        before: String,
        after: String,
    },
    MapSet {
        section: String,
        key: String,
        entry: String,
        before: Option<String>,
        after: String,
    },
    Insert {
        section: String,
        key: String,
        item: String,
    },
    Replace {
        section: String,
        key: String,
        old: String,
        new: String,
    },
    File {
        path: PathBuf,
        backup: Option<PathBuf>,
        hash: u64,
    },
    Link {
        path: PathBuf,
        target: PathBuf,
    },
    /// System-wide assets installed through the privileged helper.
    System,
}

impl Op {
    fn config_target(&self) -> Option<(&str, &str)> {
        match self {
            Self::Set { section, key, .. }
            | Self::MapSet { section, key, .. }
            | Self::Insert { section, key, .. }
            | Self::Replace { section, key, .. } => Some((section, key)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Applying,
    Applied,
    Restoring,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Journal {
    pub entity: String,
    pub phase: Phase,
    pub ops: Vec<Op>,
}

impl Journal {
    pub fn new(entity: &str) -> Self {
        Self {
            entity: entity.to_owned(),
            phase: Phase::Applying,
            ops: Vec::new(),
        }
    }

    pub fn dir() -> PathBuf {
        state_root().join("journal")
    }

    pub fn backup_dir(entity: &str) -> PathBuf {
        state_root().join("backup").join(entity)
    }

    fn path(entity: &str) -> PathBuf {
        Self::dir().join(format!("{entity}.ron"))
    }

    pub fn load(entity: &str) -> Option<Self> {
        let text = std::fs::read_to_string(Self::path(entity)).ok()?;
        ron::from_str(&text).ok()
    }

    /// Every journal on disk, for crash recovery at startup.
    pub fn load_all() -> Vec<Self> {
        let Ok(entries) = std::fs::read_dir(Self::dir()) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name();
                let entity = name.to_str()?.strip_suffix(".ron")?.to_owned();
                Self::load(&entity)
            })
            .collect()
    }

    pub fn save(&self) -> io::Result<()> {
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(io::Error::other)?;
        crate::util::fsx::write_atomic(&Self::path(&self.entity), text.as_bytes())
    }

    pub fn remove(entity: &str) {
        let _ = std::fs::remove_file(Self::path(entity));
        let _ = std::fs::remove_dir_all(Self::backup_dir(entity));
    }
}

#[cfg(test)]
mod tests;
