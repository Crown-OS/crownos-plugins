use std::collections::BTreeMap;

use anyhow::{Context, Result};
use crownos_config::{DynSection, PatchError};

use super::{Journal, Op, Phase};
use crate::util::{hash::fnv1a, raw};

fn patch_error(error: impl std::fmt::Display) -> PatchError {
    PatchError::Io(std::io::Error::other(error.to_string()))
}

/// Undo `journal`, newest change first. A value is put back only if it still
/// holds what the entity wrote; anything the user changed since is kept.
///
/// `others` are the journals of entities that stay active. When one of them
/// overwrote a value this entity wrote, its record of the value it replaced
/// is spliced to point past this entity, so disabling it later restores
/// the user's own value rather than this entity's stale one.
pub fn restore(
    journal: &mut Journal,
    others: &mut [Journal],
    system: &mut dyn FnMut(bool) -> Result<()>,
) -> Result<()> {
    journal.phase = Phase::Restoring;
    journal.save()?;
    splice(journal, others)?;

    let mut by_section: BTreeMap<String, Vec<&Op>> = BTreeMap::new();
    for op in journal.ops.iter().rev() {
        match op.config_target() {
            Some((section, _)) => by_section.entry(section.to_owned()).or_default().push(op),
            None => undo_file(op, system)?,
        }
    }
    for (section, ops) in by_section {
        crownos_config::schema::edit_section(&section, |fields| {
            ops.iter().try_for_each(|op| undo(fields, op))
        })
        .with_context(|| format!("restoring {section}.ron"))?;
    }
    Journal::remove(&journal.entity);
    Ok(())
}

fn splice(journal: &Journal, others: &mut [Journal]) -> Result<()> {
    for other in others.iter_mut() {
        let mut changed = false;
        for op in &mut other.ops {
            for mine in &journal.ops {
                changed |= splice_one(op, mine);
            }
        }
        if changed {
            other.save()?;
        }
    }
    Ok(())
}

fn splice_one(theirs: &mut Op, mine: &Op) -> bool {
    match (theirs, mine) {
        (
            Op::Set { section, key, before, .. },
            Op::Set { section: my_section, key: my_key, before: my_before, after: my_after },
        ) if section == my_section && key == my_key && raw::same(before, my_after) => {
            *before = my_before.clone();
            true
        }
        (
            Op::MapSet { section, key, entry, before, .. },
            Op::MapSet {
                section: my_section,
                key: my_key,
                entry: my_entry,
                before: my_before,
                after: my_after,
            },
        ) if section == my_section
            && key == my_key
            && entry == my_entry
            && before.as_deref().is_some_and(|before| raw::same(before, my_after)) =>
        {
            *before = my_before.clone();
            true
        }
        _ => false,
    }
}

fn undo(fields: &mut dyn DynSection, op: &Op) -> Result<(), PatchError> {
    match op {
        Op::Set { key, before, after, .. } => {
            if raw::same(&fields.get_ron(key)?, after) {
                fields.set_ron(key, before)?;
            }
        }
        Op::MapSet { key, entry, before, after, .. } => {
            let mut map = raw::parse_map(&fields.get_ron(key)?).map_err(patch_error)?;
            if map.get(entry).is_some_and(|current| raw::same(current.get_ron(), after)) {
                match before {
                    Some(before) => {
                        map.insert(entry.clone(), raw::raw(before).map_err(patch_error)?);
                    }
                    None => {
                        map.remove(entry);
                    }
                }
                fields.set_ron(key, &raw::map_text(&map))?;
            }
        }
        Op::Insert { key, item, .. } => {
            let mut items = raw::parse_list(&fields.get_ron(key)?).map_err(patch_error)?;
            if let Some(index) = raw::position(&items, item) {
                items.remove(index);
                fields.set_ron(key, &raw::list_text(&items))?;
            }
        }
        Op::Replace { key, old, new, .. } => {
            let mut items = raw::parse_list(&fields.get_ron(key)?).map_err(patch_error)?;
            if let Some(index) = raw::position(&items, new) {
                items[index] = raw::raw(old).map_err(patch_error)?;
                fields.set_ron(key, &raw::list_text(&items))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn undo_file(op: &Op, system: &mut dyn FnMut(bool) -> Result<()>) -> Result<()> {
    match op {
        Op::File { path, backup, hash } => {
            let ours = std::fs::read(path).is_ok_and(|bytes| fnv1a(&bytes) == *hash);
            if ours {
                match backup {
                    Some(backup) => std::fs::rename(backup, path)?,
                    None => std::fs::remove_file(path)?,
                }
            }
        }
        Op::Link { path, target } => {
            if std::fs::read_link(path).ok().as_deref() == Some(target.as_path()) {
                std::fs::remove_file(path)?;
            }
        }
        Op::System => system(false)?,
        _ => {}
    }
    Ok(())
}
