pub use crownos_config::schema::LockSlotPosition;
use crownos_config::schema::WindowRule;
use serde::{Deserialize, Serialize};

use super::{ConfigPatch, api_v1};
use crate::settings::SettingsSection;
use crate::{Capability, EntityId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "Plugin")]
pub struct PluginManifest {
    pub id: EntityId,
    pub name: String,
    pub version: String,
    #[serde(default = "api_v1")]
    pub api: u32,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub license: String,
    /// The Luau script that runs the plugin.
    #[serde(default = "default_entry")]
    pub entry: String,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub contributes: Contributes,
    #[serde(default)]
    pub limits: Limits,
}

fn default_entry() -> String {
    "main.luau".to_owned()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Contributes {
    pub bar: Vec<BarWidgetDecl>,
    pub lock: Vec<LockSlotDecl>,
    pub settings: Option<SettingsSection>,
    pub keybinds: Vec<KeybindDecl>,
    pub window_rules: Vec<WindowRule>,
    pub config: ConfigPatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BarSlot {
    Left,
    Center,
    #[default]
    Right,
}

impl BarSlot {
    pub const fn config_key(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BarWidgetDecl {
    pub id: String,
    #[serde(default)]
    pub slot: BarSlot,
    /// Place right after this entry (a built-in name or `plugin:<id>/<widget>`).
    #[serde(default)]
    pub after: Option<String>,
    /// Take this built-in's place instead of adding a new pill.
    #[serde(default)]
    pub replaces: Option<String>,
}

impl BarWidgetDecl {
    pub fn entry(&self, plugin: &EntityId) -> String {
        format!("{}{plugin}/{}", crownos_config::schema::bar::PLUGIN_PREFIX, self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockSlotDecl {
    pub id: String,
    #[serde(default)]
    pub slot: LockSlotPosition,
}

/// A global shortcut that calls the plugin's `actions[action]` handler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeybindDecl {
    pub keys: String,
    pub action: String,
}

impl KeybindDecl {
    /// The compositor action string this binding is written as.
    pub fn compositor_action(&self, plugin: &EntityId) -> String {
        format!("plugin {plugin} {}", self.action)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Limits {
    pub memory_mb: u32,
    /// How long one callback may run before it is interrupted.
    pub call_budget_ms: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_mb: 16,
            call_budget_ms: 20,
        }
    }
}
