use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use crownos_config::{DynSection, PatchError};

use super::{Journal, Op, Phase};
use crate::util::{fsx, hash::fnv1a, raw};

/// One change an entity wants made.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Set {
        section: String,
        key: String,
        value: String,
    },
    MapSet {
        section: String,
        key: String,
        entry: String,
        value: String,
    },
    /// Add `item` to a list, right after `after` when that is present.
    Insert {
        section: String,
        key: String,
        item: String,
        after: Option<String>,
    },
    Replace {
        section: String,
        key: String,
        old: String,
        new: String,
    },
    File {
        path: PathBuf,
        contents: Vec<u8>,
    },
    Link {
        path: PathBuf,
        target: PathBuf,
    },
    System,
}

impl Change {
    fn section(&self) -> Option<&str> {
        match self {
            Self::Set { section, .. }
            | Self::MapSet { section, .. }
            | Self::Insert { section, .. }
            | Self::Replace { section, .. } => Some(section),
            _ => None,
        }
    }
}

fn patch_error(error: impl std::fmt::Display) -> PatchError {
    PatchError::Io(std::io::Error::other(error.to_string()))
}

/// Make `changes` and record them in `journal`, saving the journal before
/// each section is written. On error the journal holds what was done so far,
/// ready for [`restore`](super::restore).
pub fn apply(
    journal: &mut Journal,
    changes: Vec<Change>,
    system: &mut dyn FnMut(bool) -> Result<()>,
) -> Result<()> {
    journal.phase = Phase::Applying;
    let mut by_section: BTreeMap<String, Vec<Change>> = BTreeMap::new();
    let mut others = Vec::new();
    for change in changes {
        match change.section() {
            Some(section) => by_section.entry(section.to_owned()).or_default().push(change),
            None => others.push(change),
        }
    }
    for (section, changes) in by_section {
        crownos_config::schema::edit_section(&section, |fields| {
            for change in &changes {
                perform(journal, fields, change)?;
            }
            journal.save()?;
            Ok(())
        })
        .with_context(|| format!("writing {section}.ron"))?;
    }
    for change in others {
        perform_file(journal, change, system)?;
    }
    journal.phase = Phase::Applied;
    journal.save()?;
    Ok(())
}

fn perform(
    journal: &mut Journal,
    fields: &mut dyn DynSection,
    change: &Change,
) -> Result<(), PatchError> {
    match change {
        Change::Set { section, key, value } => {
            let before = fields.get_ron(key)?;
            fields.set_ron(key, value)?;
            let after = fields.get_ron(key)?;
            let existing = journal.ops.iter_mut().find_map(|op| match op {
                Op::Set { section: s, key: k, after: a, .. } if s == section && k == key => Some(a),
                _ => None,
            });
            match existing {
                Some(previous) => *previous = after,
                None => journal.ops.push(Op::Set {
                    section: section.clone(),
                    key: key.clone(),
                    before,
                    after,
                }),
            }
        }
        Change::MapSet { section, key, entry, value } => {
            let mut map = raw::parse_map(&fields.get_ron(key)?).map_err(patch_error)?;
            let before = map.get(entry).map(|value| value.get_ron().to_owned());
            map.insert(entry.clone(), raw::raw(value).map_err(patch_error)?);
            fields.set_ron(key, &raw::map_text(&map))?;
            let after = raw::parse_map(&fields.get_ron(key)?)
                .map_err(patch_error)?
                .get(entry)
                .map(|value| value.get_ron().to_owned())
                .unwrap_or_default();
            let existing = journal.ops.iter_mut().find_map(|op| match op {
                Op::MapSet { section: s, key: k, entry: e, after: a, .. }
                    if s == section && k == key && e == entry =>
                {
                    Some(a)
                }
                _ => None,
            });
            match existing {
                Some(previous) => *previous = after,
                None => journal.ops.push(Op::MapSet {
                    section: section.clone(),
                    key: key.clone(),
                    entry: entry.clone(),
                    before,
                    after,
                }),
            }
        }
        Change::Insert { section, key, item, after } => {
            let mut items = raw::parse_list(&fields.get_ron(key)?).map_err(patch_error)?;
            if raw::position(&items, item).is_some() {
                return Ok(());
            }
            let at = after
                .as_deref()
                .and_then(|after| raw::position(&items, after))
                .map_or(items.len(), |index| index + 1);
            items.insert(at, raw::raw(item).map_err(patch_error)?);
            fields.set_ron(key, &raw::list_text(&items))?;
            journal.ops.push(Op::Insert {
                section: section.clone(),
                key: key.clone(),
                item: item.clone(),
            });
        }
        Change::Replace { section, key, old, new } => {
            let mut items = raw::parse_list(&fields.get_ron(key)?).map_err(patch_error)?;
            let Some(index) = raw::position(&items, old) else {
                return Ok(());
            };
            items[index] = raw::raw(new).map_err(patch_error)?;
            fields.set_ron(key, &raw::list_text(&items))?;
            journal.ops.push(Op::Replace {
                section: section.clone(),
                key: key.clone(),
                old: old.clone(),
                new: new.clone(),
            });
        }
        _ => {}
    }
    Ok(())
}

fn perform_file(
    journal: &mut Journal,
    change: Change,
    system: &mut dyn FnMut(bool) -> Result<()>,
) -> Result<()> {
    match change {
        Change::File { path, contents } => {
            let hash = fnv1a(&contents);
            let recorded = journal.ops.iter_mut().find_map(|op| match op {
                Op::File { path: recorded, hash, .. } if *recorded == path => Some(hash),
                _ => None,
            });
            match recorded {
                Some(previous) => *previous = hash,
                None => {
                    let backup = path
                        .exists()
                        .then(|| backup_existing(&journal.entity, &path))
                        .transpose()?;
                    journal.ops.push(Op::File { path: path.clone(), backup, hash });
                }
            }
            journal.save()?;
            fsx::write_atomic(&path, &contents)
                .with_context(|| format!("writing {}", path.display()))?;
        }
        Change::Link { path, target } => {
            if std::fs::symlink_metadata(&path).is_ok() {
                if std::fs::read_link(&path).ok().as_deref() == Some(target.as_path()) {
                    return Ok(());
                }
                return Err(anyhow!("{} already exists", path.display()));
            }
            journal.ops.push(Op::Link { path: path.clone(), target: target.clone() });
            journal.save()?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::os::unix::fs::symlink(&target, &path)
                .with_context(|| format!("linking {}", path.display()))?;
        }
        Change::System => {
            journal.ops.push(Op::System);
            journal.save()?;
            system(true)?;
        }
        _ => {}
    }
    Ok(())
}

fn backup_existing(entity: &str, path: &std::path::Path) -> Result<PathBuf> {
    let backup = Journal::backup_dir(entity)
        .join(path.to_string_lossy().trim_start_matches('/').replace('/', "%"));
    if let Some(parent) = backup.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(path, &backup).with_context(|| format!("backing up {}", path.display()))?;
    Ok(backup)
}
