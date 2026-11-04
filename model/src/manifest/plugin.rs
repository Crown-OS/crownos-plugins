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
    pub launcher: Vec<LauncherWidgetDecl>,
    pub search: Vec<SearchProviderDecl>,
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

/// How much of the launcher's grid a widget takes: columns across, and a
/// short row (a strip) or a tall one (a square).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LauncherSize {
    /// One column, one short row.
    #[default]
    Small,
    /// Two columns, one short row.
    Wide,
    /// One column, one tall row.
    Square,
    /// Two columns, one tall row.
    Large,
}

impl LauncherSize {
    pub const fn columns(self) -> u8 {
        match self {
            Self::Small | Self::Square => 1,
            Self::Wide | Self::Large => 2,
        }
    }

    pub const fn is_tall(self) -> bool {
        matches!(self, Self::Square | Self::Large)
    }
}

/// A tile on the launcher's home grid, drawn with `ui.render(id, tree)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LauncherWidgetDecl {
    pub id: String,
    #[serde(default)]
    pub size: LauncherSize,
}

impl LauncherWidgetDecl {
    /// The entry this widget is listed under in `launcher.ron`.
    pub fn entry(&self, plugin: &EntityId) -> String {
        format!("plugin:{plugin}/{}", self.id)
    }
}

/// A source of launcher search results, answered by `on_search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchProviderDecl {
    pub id: String,
    /// The heading its results are grouped under.
    pub name: String,
    /// Only queries starting with this word are sent, without it; `None`
    /// sends every query.
    #[serde(default)]
    pub keyword: Option<String>,
}

impl SearchProviderDecl {
    /// What the provider is asked for `query`, or `None` when it does not
    /// answer that query.
    pub fn accepts<'q>(&self, query: &'q str) -> Option<&'q str> {
        let query = query.trim_start();
        match &self.keyword {
            None => Some(query),
            Some(keyword) => {
                let rest = query.strip_prefix(keyword.as_str())?;
                rest.starts_with(char::is_whitespace).then(|| rest.trim_start())
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(keyword: Option<&str>) -> SearchProviderDecl {
        SearchProviderDecl {
            id: "calc".into(),
            name: "Calculator".into(),
            keyword: keyword.map(str::to_owned),
        }
    }

    #[test]
    fn a_keyword_provider_hears_only_its_own_queries() {
        let keyed = provider(Some("gh"));
        assert_eq!(keyed.accepts("gh  crownos"), Some("crownos"));
        assert_eq!(keyed.accepts("ghost"), None);
        assert_eq!(keyed.accepts("gh"), None);
        assert_eq!(provider(None).accepts("  2+2"), Some("2+2"));
    }

    #[test]
    fn sizes_span_the_grid() {
        assert_eq!(LauncherSize::Large.columns(), 2);
        assert!(LauncherSize::Square.is_tall() && !LauncherSize::Wide.is_tall());
    }
}
