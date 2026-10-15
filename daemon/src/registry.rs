//! Every installed theme and plugin, plus the ones loaded in dev mode.

use std::collections::{BTreeMap, VecDeque};

use crownplugin_model::package::scan;
use crownplugin_model::{EntityId, Kind, Package};
use crownplugin_proto::{EntityStatus, LogLevel, LogLine};
use tracing::warn;

const LOG_LINES: usize = 200;

pub struct Entry {
    pub package: Package,
    pub dev: bool,
    pub status: EntityStatus,
    pub logs: VecDeque<LogLine>,
}

impl Entry {
    fn new(package: Package, dev: bool) -> Self {
        Self {
            package,
            dev,
            status: EntityStatus::Inactive,
            logs: VecDeque::new(),
        }
    }

    pub fn kind(&self) -> Kind {
        self.package.manifest.kind()
    }

    pub fn log(&mut self, level: LogLevel, message: String) -> LogLine {
        let line = LogLine {
            id: self.package.id().to_string(),
            level,
            at: now_millis(),
            message,
        };
        if self.logs.len() == LOG_LINES {
            self.logs.pop_front();
        }
        self.logs.push_back(line.clone());
        line
    }
}

pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

#[derive(Default)]
pub struct Registry {
    entries: BTreeMap<String, Entry>,
}

impl Registry {
    /// Re-read the install directories, keeping dev entries, statuses and logs.
    pub fn rescan(&mut self) {
        let mut fresh: BTreeMap<String, Entry> = BTreeMap::new();
        for kind in [Kind::Theme, Kind::Plugin] {
            for package in scan(kind) {
                match package {
                    Ok(package) => {
                        fresh.insert(package.id().to_string(), Entry::new(package, false));
                    }
                    Err(error) => warn!("skipping a package: {error}"),
                }
            }
        }
        for (id, old) in std::mem::take(&mut self.entries) {
            match fresh.get_mut(&id) {
                Some(entry) if !old.dev => {
                    entry.status = old.status;
                    entry.logs = old.logs;
                }
                _ if old.dev => {
                    fresh.insert(id, old);
                }
                _ => {}
            }
        }
        self.entries = fresh;
    }

    pub fn insert_dev(&mut self, package: Package) -> EntityId {
        let id = package.id().clone();
        let logs = self.entries.remove(id.as_str()).map(|old| old.logs).unwrap_or_default();
        let mut entry = Entry::new(package, true);
        entry.logs = logs;
        self.entries.insert(id.to_string(), entry);
        id
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Entry> {
        self.entries.get_mut(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<Entry> {
        self.entries.remove(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }
}
