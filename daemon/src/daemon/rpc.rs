//! The `crownplugind` protocol, served from the daemon's loop.

use crownos_config::schema::Decision;
use crownos_ipc::RemoteError;
use crownplugin_model::{Manifest, SettingValue};
use crownplugin_proto::plugind::Handler;
use crownplugin_proto::{
    CapabilityInfo, EntityInfo, HitKey, Host, Kind, LauncherSnapshot, LogLine, Part, Snapshot, Source, SurfaceKey,
    UiEvent,
};

use super::{Daemon, Message};
use crate::store;

fn not_found(id: &str) -> RemoteError {
    RemoteError::handler(format!("`{id}` is not installed"))
}

fn failed(error: anyhow::Error) -> RemoteError {
    RemoteError::handler(format!("{error:#}"))
}

impl Daemon {
    fn info(&self, entry: &crate::registry::Entry) -> EntityInfo {
        let manifest = &entry.package.manifest;
        let id = manifest.id().as_str();
        let active = match manifest.kind() {
            Kind::Theme => self.store.active_theme() == Some(id),
            Kind::Plugin => self.store.is_enabled(id),
        };
        EntityInfo {
            id: id.to_owned(),
            kind: manifest.kind(),
            name: manifest.name().to_owned(),
            version: manifest.version().to_owned(),
            author: manifest.author().to_owned(),
            description: manifest.description().to_owned(),
            dir: entry.package.dir.to_string_lossy().into_owned(),
            system: entry.package.system,
            active,
            dev: entry.dev,
            status: entry.status.clone(),
            capabilities: manifest
                .capabilities()
                .iter()
                .map(|capability| {
                    let key = capability.grant_key().into_owned();
                    CapabilityInfo {
                        decision: self.store.decision(id, &key),
                        summary: capability.summary(),
                        level: capability.level(),
                        key,
                    }
                })
                .collect(),
            settings: match manifest {
                Manifest::Plugin(plugin) => plugin.contributes.settings.clone(),
                Manifest::Theme(_) => None,
            },
        }
    }
}

impl Handler for Daemon {
    fn attach(&mut self, host: Host) -> Result<Snapshot, RemoteError> {
        Ok(match host {
            Host::Bar => self.surfaces.bar(),
            Host::Lock => self.surfaces.lock(),
            Host::Settings | Host::Cli => Snapshot::default(),
        })
    }

    fn ui_event(&mut self, surface: SurfaceKey, rev: u32, event: UiEvent) {
        if self.surfaces.current_rev(&surface.plugin, &surface.widget, surface.part) != Some(rev) {
            return;
        }
        let _ = self.sender.send(Message::UiEvent {
            plugin: surface.plugin,
            widget: surface.widget,
            part: surface.part,
            event,
        });
    }

    fn popup(&mut self, surface: SurfaceKey, open: bool) {
        if surface.part != Part::Popup {
            return;
        }
        if let Some(running) = self.running.get_mut(&surface.plugin) {
            let outcome = running.vm.popup(&surface.widget, open);
            self.after_call(&surface.plugin, outcome);
        }
    }

    fn attach_launcher(&mut self) -> Result<LauncherSnapshot, RemoteError> {
        Ok(self.surfaces.launcher())
    }

    fn launcher_shown(&mut self, shown: bool) {
        let ids: Vec<String> = self.running.keys().cloned().collect();
        for id in ids {
            if let Some(running) = self.running.get_mut(&id) {
                let outcome = running.vm.launcher_shown(shown);
                self.after_call(&id, outcome);
            }
        }
    }

    fn search(&mut self, query: String) {
        self.search_query = query;
        let asked: Vec<(String, String, String)> = self
            .registry
            .iter()
            .filter(|entry| self.running.contains_key(entry.package.manifest.id().as_str()))
            .filter_map(|entry| match &entry.package.manifest {
                Manifest::Plugin(plugin) => Some(plugin),
                Manifest::Theme(_) => None,
            })
            .flat_map(|plugin| {
                plugin.contributes.search.iter().filter_map(|provider| {
                    let asked = provider.accepts(&self.search_query)?;
                    (!asked.is_empty()).then(|| (plugin.id.to_string(), provider.id.clone(), asked.to_owned()))
                })
            })
            .collect();
        for (id, provider, query) in asked {
            if let Some(running) = self.running.get_mut(&id) {
                let outcome = running.vm.search(&provider, &query);
                self.after_call(&id, outcome);
            }
        }
    }

    fn activate(&mut self, hit: HitKey) {
        if let Some(running) = self.running.get_mut(&hit.plugin) {
            let outcome = running.vm.activate(&hit.provider, &hit.id);
            self.after_call(&hit.plugin, outcome);
        }
    }

    fn list(&mut self) -> Result<Vec<EntityInfo>, RemoteError> {
        Ok(self.registry.iter().map(|entry| self.info(entry)).collect())
    }

    fn install(&mut self, source: Source) -> Result<String, RemoteError> {
        self.install_from(&source).map_err(failed)
    }

    fn uninstall(&mut self, id: String) -> Result<(), RemoteError> {
        self.uninstall_entity(&id).map_err(failed)
    }

    fn set_enabled(&mut self, id: String, enabled: bool) -> Result<(), RemoteError> {
        let entry = self.registry.get(&id).ok_or_else(|| not_found(&id))?;
        match entry.kind() {
            Kind::Plugin => self.store.set_enabled(&id, enabled),
            Kind::Theme => self.store.set_active_theme(enabled.then_some(id.as_str())),
        }
        self.reconcile();
        Ok(())
    }

    fn set_theme(&mut self, id: Option<String>) -> Result<(), RemoteError> {
        if let Some(id) = &id {
            let entry = self.registry.get(id).ok_or_else(|| not_found(id))?;
            if entry.kind() != Kind::Theme {
                return Err(RemoteError::handler(format!("`{id}` is not a theme")));
            }
        }
        self.store.set_active_theme(id.as_deref());
        self.reconcile();
        Ok(())
    }

    fn set_grant(&mut self, id: String, capability: String, decision: Option<Decision>) -> Result<(), RemoteError> {
        self.registry.get(&id).ok_or_else(|| not_found(&id))?;
        self.store.set_decision(&id, &capability, decision);
        self.restart_vm(&id);
        self.outbox.entities = true;
        self.schedule_flush();
        Ok(())
    }

    fn settings(&mut self, id: String) -> Result<Vec<(String, SettingValue)>, RemoteError> {
        let entry = self.registry.get(&id).ok_or_else(|| not_found(&id))?;
        let Manifest::Plugin(plugin) = &entry.package.manifest else {
            return Ok(Vec::new());
        };
        let Some(section) = &plugin.contributes.settings else {
            return Ok(Vec::new());
        };
        Ok(section.resolve(&store::load_settings(&plugin.id)).into_iter().collect())
    }

    fn set_setting(&mut self, id: String, key: String, value: SettingValue) -> Result<(), RemoteError> {
        let entry = self.registry.get(&id).ok_or_else(|| not_found(&id))?;
        let Manifest::Plugin(plugin) = &entry.package.manifest else {
            return Err(RemoteError::handler("themes have no settings"));
        };
        let field = plugin
            .contributes
            .settings
            .as_ref()
            .and_then(|section| section.field(&key))
            .ok_or_else(|| RemoteError::handler(format!("`{key}` is not one of {id}'s settings")))?;
        if !field.accepts(&value) {
            return Err(RemoteError::handler(format!("{value:?} does not fit `{key}`")));
        }
        let mut values = store::load_settings(&plugin.id);
        values.insert(key, value);
        store::save_settings(&plugin.id, &values);
        self.handle(Message::SettingsEdited(id));
        Ok(())
    }

    fn logs(&mut self, id: String) -> Result<Vec<LogLine>, RemoteError> {
        let entry = self.registry.get(&id).ok_or_else(|| not_found(&id))?;
        Ok(entry.logs.iter().cloned().collect())
    }

    fn dev_load(&mut self, path: String) -> Result<String, RemoteError> {
        self.dev_load_dir(std::path::Path::new(&path)).map_err(failed)
    }

    fn reload(&mut self, id: String) -> Result<(), RemoteError> {
        self.registry.get(&id).ok_or_else(|| not_found(&id))?;
        self.reload(&id);
        Ok(())
    }
}
