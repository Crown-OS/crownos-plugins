//! Turning themes and plugins on and off.

use std::collections::BTreeSet;
use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use crownplugin_model::{Capability, Manifest};
use crownplugin_proto::{EntityStatus, LogLevel};
use tracing::{info, warn};

use super::{Daemon, Running};
use crate::changes;
use crate::journal::{self, Change, Journal, Phase};
use crate::store;
use crate::theme::system;
use crate::vm::{Grants, Outcome, Seed, Vm};

impl Daemon {
    /// The ids that should be active according to `plugins.ron`.
    pub fn desired(&self) -> BTreeSet<String> {
        let plugins = self.store.plugins();
        let mut desired: BTreeSet<String> = plugins
            .enabled
            .iter()
            .filter(|id| {
                self.registry
                    .get(id)
                    .is_some_and(|entry| matches!(entry.package.manifest, Manifest::Plugin(_)))
            })
            .cloned()
            .collect();
        if let Some(theme) = &plugins.active_theme
            && self
                .registry
                .get(theme)
                .is_some_and(|entry| matches!(entry.package.manifest, Manifest::Theme(_)))
        {
            desired.insert(theme.clone());
        }
        desired
    }

    fn active(&self) -> BTreeSet<String> {
        Journal::load_all()
            .into_iter()
            .map(|journal| journal.entity)
            .chain(self.running.keys().cloned())
            .collect()
    }

    /// Undo whatever a crash left half-done, then match `plugins.ron`.
    pub fn recover(&mut self) {
        let desired = self.desired();
        for journal in Journal::load_all() {
            if journal.phase != Phase::Applied || !desired.contains(&journal.entity) {
                info!("undoing leftover changes of {}", journal.entity);
                self.restore_journal(&journal.entity);
            }
        }
        self.reconcile();
    }

    pub fn reconcile(&mut self) {
        let desired = self.desired();
        let active = self.active();
        for id in active.difference(&desired) {
            self.deactivate(id);
        }
        for id in desired.difference(&active) {
            self.activate(id);
        }
        for id in &desired {
            if !self.running.contains_key(id)
                && self.registry.get(id).is_some_and(|entry| entry.status == EntityStatus::Inactive)
            {
                self.start_vm(id);
            }
        }
        self.outbox.entities = true;
        self.schedule_flush();
    }

    pub fn activate(&mut self, id: &str) {
        let Some(entry) = self.registry.get(id) else {
            return;
        };
        let package = entry.package.clone();
        let result = (|| -> Result<Vec<String>> {
            let (changes, warnings) = match &package.manifest {
                Manifest::Theme(theme) => (changes::theme_changes(&package, theme)?, Vec::new()),
                Manifest::Plugin(plugin) => changes::plugin_changes(plugin)?,
            };
            self.apply_changes(id, changes)?;
            if matches!(&package.manifest, Manifest::Theme(theme) if !theme.assets.fonts.is_empty()) {
                refresh_font_cache();
            }
            Ok(warnings)
        })();
        match result {
            Ok(warnings) => {
                for warning in warnings {
                    self.log(id, LogLevel::Warn, warning);
                }
                self.log(id, LogLevel::Info, "enabled".into());
                self.start_vm(id);
            }
            Err(error) => {
                self.log(id, LogLevel::Error, format!("could not enable: {error:#}"));
                self.restore_journal(id);
                self.set_status(id, EntityStatus::Error(format!("{error:#}")));
            }
        }
    }

    pub fn deactivate(&mut self, id: &str) {
        self.stop_vm(id);
        self.restore_journal(id);
        self.set_status(id, EntityStatus::Inactive);
        self.log(id, LogLevel::Info, "disabled".into());
    }

    /// Record and make `changes` on behalf of `id`.
    pub fn apply_changes(&mut self, id: &str, changes: Vec<Change>) -> Result<()> {
        let dir = self.registry.get(id).map(|entry| entry.package.dir.clone());
        let mut journal = Journal::load(id).unwrap_or_else(|| Journal::new(id));
        let mut system = |install: bool| {
            let dir = dir.as_deref().ok_or_else(|| anyhow!("{id} is not installed"))?;
            system::spawn(install, id, dir);
            Ok(())
        };
        journal::apply(&mut journal, changes, &mut system).with_context(|| format!("applying {id}"))
    }

    pub fn restore_journal(&mut self, id: &str) {
        let Some(mut journal) = Journal::load(id) else {
            return;
        };
        let mut others: Vec<Journal> = Journal::load_all()
            .into_iter()
            .filter(|other| other.entity != id)
            .collect();
        let dir = self.registry.get(id).map(|entry| entry.package.dir.clone());
        let mut system = |install: bool| {
            if let Some(dir) = &dir {
                system::spawn(install, id, dir);
            }
            Ok(())
        };
        if let Err(error) = journal::restore(&mut journal, &mut others, &mut system) {
            warn!("restoring {id}: {error:#}");
        }
    }

    pub fn grants(&self, id: &str) -> Grants {
        let Some(entry) = self.registry.get(id) else {
            return Grants::default();
        };
        let declared: Vec<Capability> = entry.package.manifest.capabilities();
        let denied = declared
            .iter()
            .map(|capability| capability.grant_key().into_owned())
            .filter(|key| {
                self.store.decision(id, key) == Some(crownos_config::schema::Decision::Denied)
            })
            .collect();
        Grants { declared, denied }
    }

    pub fn start_vm(&mut self, id: &str) {
        let Some(entry) = self.registry.get(id) else {
            return;
        };
        if entry.package.manifest.script().is_none() {
            self.set_status(id, EntityStatus::Running);
            return;
        }
        let package = entry.package.clone();
        let grants = self.grants(id);
        let seed = Seed {
            dark_mode: self.dark_mode,
            tokens: self.tokens(),
            settings: match &package.manifest {
                Manifest::Plugin(plugin) => plugin
                    .contributes
                    .settings
                    .as_ref()
                    .map(|section| section.resolve(&store::load_settings(package.id())))
                    .unwrap_or_default(),
                Manifest::Theme(_) => Default::default(),
            },
        };
        match Vm::start(&package, &grants, seed) {
            Ok((vm, outcome)) => {
                let settings_watch = self.watches.settings(package.id());
                self.running.insert(
                    id.to_owned(),
                    Running { vm, timers: HashMap::new(), _settings_watch: settings_watch },
                );
                self.set_status(id, EntityStatus::Running);
                self.after_call(id, outcome);
            }
            Err(error) => {
                self.log(id, LogLevel::Error, format!("could not start: {error:#}"));
                self.set_status(id, EntityStatus::Error(format!("{error:#}")));
            }
        }
    }

    pub fn stop_vm(&mut self, id: &str) {
        if let Some(mut running) = self.running.remove(id) {
            running.vm.unload();
            for token in running.timers.into_values() {
                self.handle.remove(token);
            }
        }
        self.parked.retain(|parked| parked.entity != id);
        let (bar, lock) = self.surfaces.remove(id);
        self.outbox.bar |= bar;
        self.outbox.lock |= lock;
        self.schedule_flush();
    }

    pub fn restart_vm(&mut self, id: &str) {
        if self.running.contains_key(id) {
            self.stop_vm(id);
            self.start_vm(id);
        }
    }

    /// Run the commands a call queued and react to how the call went.
    pub fn after_call(&mut self, id: &str, outcome: Outcome) {
        match outcome {
            Outcome::Ok => {}
            Outcome::Failed(message) => self.log(id, LogLevel::Error, message),
            Outcome::Quarantine(message) => {
                self.log(id, LogLevel::Error, format!("{message}; stopped after repeated failures"));
                self.stop_vm(id);
                self.set_status(id, EntityStatus::Quarantined(message));
                return;
            }
        }
        let commands = match self.running.get(id) {
            Some(running) => running.vm.take_commands(),
            None => return,
        };
        for command in commands {
            self.run_command(id, command);
        }
    }

    pub fn set_status(&mut self, id: &str, status: crownplugin_proto::EntityStatus) {
        if let Some(entry) = self.registry.get_mut(id)
            && entry.status != status
        {
            entry.status = status;
            self.outbox.entities = true;
            self.schedule_flush();
        }
    }

    pub fn log(&mut self, id: &str, level: LogLevel, message: String) {
        match level {
            LogLevel::Error | LogLevel::Warn => warn!("{id}: {message}"),
            LogLevel::Info | LogLevel::Debug => info!("{id}: {message}"),
        }
        if let Some(entry) = self.registry.get_mut(id) {
            let line = entry.log(level, message);
            self.outbox.logs.push(line);
            self.schedule_flush();
        }
    }

    /// The active mode's token overrides.
    pub fn tokens(&self) -> std::collections::BTreeMap<String, String> {
        let theme: crownos_config::Theme = crownos_config::load("theme");
        theme.slots(self.dark_mode).clone()
    }
}

fn refresh_font_cache() {
    std::thread::spawn(|| std::process::Command::new("fc-cache").arg("-f").status());
}
