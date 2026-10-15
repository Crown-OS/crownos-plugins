//! `plugins.ron` and the per-plugin `plugin.<id>.ron` files. crownplugind is
//! the only process that writes them.

use std::collections::BTreeMap;

use crownos_config::schema::{Decision, Plugins};
use crownplugin_model::{EntityId, SettingValue};
use tracing::warn;

pub struct Store {
    plugins: Plugins,
}

impl Store {
    pub fn load() -> Self {
        let plugins = crownos_config::load_strict(Plugins::SECTION).unwrap_or_else(|error| {
            warn!("plugins.ron is unreadable, starting with nothing enabled: {error}");
            Plugins::default()
        });
        Self { plugins }
    }

    pub fn plugins(&self) -> &Plugins {
        &self.plugins
    }

    /// Adopt a version edited by hand.
    pub fn replace(&mut self, plugins: Plugins) {
        self.plugins = plugins;
    }

    fn save(&self) {
        if let Err(error) = crownos_config::save(Plugins::SECTION, &self.plugins) {
            warn!("could not save plugins.ron: {error}");
        }
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.plugins.enabled.iter().any(|enabled| enabled == id)
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) {
        self.plugins.enabled.retain(|existing| existing != id);
        if enabled {
            self.plugins.enabled.push(id.to_owned());
        }
        self.save();
    }

    pub fn active_theme(&self) -> Option<&str> {
        self.plugins.active_theme.as_deref()
    }

    pub fn set_active_theme(&mut self, id: Option<&str>) {
        self.plugins.active_theme = id.map(str::to_owned);
        self.save();
    }

    pub fn decision(&self, id: &str, capability: &str) -> Option<Decision> {
        self.plugins.decision(id, capability)
    }

    pub fn set_decision(&mut self, id: &str, capability: &str, decision: Option<Decision>) {
        match decision {
            Some(decision) => {
                self.plugins
                    .grants
                    .entry(id.to_owned())
                    .or_default()
                    .insert(capability.to_owned(), decision);
            }
            None => {
                if let Some(grants) = self.plugins.grants.get_mut(id) {
                    grants.remove(capability);
                }
            }
        }
        self.plugins.grants.retain(|_, grants| !grants.is_empty());
        self.save();
    }

    /// Drop everything recorded about an uninstalled entity.
    pub fn forget(&mut self, id: &EntityId) {
        self.plugins.enabled.retain(|existing| existing != id.as_str());
        self.plugins.grants.remove(id.as_str());
        if self.active_theme() == Some(id.as_str()) {
            self.plugins.active_theme = None;
        }
        self.save();
        let _ = std::fs::remove_file(crownos_config::path_for(&id.settings_section()));
    }
}

pub fn load_settings(id: &EntityId) -> BTreeMap<String, SettingValue> {
    crownos_config::load_strict(&id.settings_section()).unwrap_or_default()
}

pub fn save_settings(id: &EntityId, values: &BTreeMap<String, SettingValue>) {
    if let Err(error) = crownos_config::save(&id.settings_section(), values) {
        warn!("could not save settings for {id}: {error}");
    }
}
