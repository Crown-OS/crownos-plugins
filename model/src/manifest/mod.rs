//! `theme.ron` and `plugin.ron`.

mod plugin;
mod ron_text;
mod theme;

use std::collections::BTreeMap;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub use plugin::{
    BarSlot, BarWidgetDecl, Contributes, KeybindDecl, Limits, LockSlotDecl, LockSlotPosition,
    PluginManifest,
};
pub use ron_text::RonText;
pub use theme::{LockTheme, ThemeAssets, ThemeManifest, Tokens, Wallpaper};

use crate::{Capability, EntityId, ModelError};

/// `section -> key -> value`, values in the section's own RON spelling.
pub type ConfigPatch = BTreeMap<String, BTreeMap<String, RonText>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Theme,
    Plugin,
}

impl Kind {
    pub const fn manifest_file(self) -> &'static str {
        match self {
            Self::Theme => "theme.ron",
            Self::Plugin => "plugin.ron",
        }
    }

    pub const fn dir_name(self) -> &'static str {
        match self {
            Self::Theme => "themes",
            Self::Plugin => "plugins",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Manifest {
    Theme(Box<ThemeManifest>),
    Plugin(Box<PluginManifest>),
}

impl Manifest {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Theme(_) => Kind::Theme,
            Self::Plugin(_) => Kind::Plugin,
        }
    }

    pub fn id(&self) -> &EntityId {
        match self {
            Self::Theme(theme) => &theme.id,
            Self::Plugin(plugin) => &plugin.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Theme(theme) => &theme.name,
            Self::Plugin(plugin) => &plugin.name,
        }
    }

    pub fn version(&self) -> &str {
        match self {
            Self::Theme(theme) => &theme.version,
            Self::Plugin(plugin) => &plugin.version,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Theme(theme) => &theme.description,
            Self::Plugin(plugin) => &plugin.description,
        }
    }

    pub fn author(&self) -> &str {
        match self {
            Self::Theme(theme) => &theme.author,
            Self::Plugin(plugin) => &plugin.author,
        }
    }

    pub fn api(&self) -> u32 {
        match self {
            Self::Theme(theme) => theme.api,
            Self::Plugin(plugin) => plugin.api,
        }
    }

    pub fn config(&self) -> &ConfigPatch {
        match self {
            Self::Theme(theme) => &theme.config,
            Self::Plugin(plugin) => &plugin.contributes.config,
        }
    }

    pub fn script(&self) -> Option<&str> {
        match self {
            Self::Theme(theme) => theme.script.as_deref(),
            Self::Plugin(plugin) => Some(&plugin.entry),
        }
    }

    /// Declared capabilities plus the safe ones its contributions imply.
    pub fn capabilities(&self) -> Vec<Capability> {
        let mut capabilities = match self {
            Self::Theme(theme) => theme.capabilities.clone(),
            Self::Plugin(plugin) => plugin.capabilities.clone(),
        };
        capabilities.extend([Capability::OwnConfig, Capability::ReadTheme]);
        match self {
            Self::Theme(_) => capabilities.push(Capability::ThemeVisual),
            Self::Plugin(plugin) => {
                let contributes = &plugin.contributes;
                if !contributes.bar.is_empty() {
                    capabilities.push(Capability::BarWidget);
                }
                if !contributes.lock.is_empty() {
                    capabilities.push(Capability::LockSlot);
                }
                if contributes.settings.is_some() {
                    capabilities.push(Capability::SettingsSection);
                }
                if contributes.config.keys().any(|section| {
                    matches!(section.as_str(), "appearance" | "theme" | "bar" | "lock")
                }) || !contributes.bar.is_empty()
                {
                    capabilities.push(Capability::ThemeVisual);
                }
                if !contributes.window_rules.is_empty() {
                    capabilities.push(Capability::WindowRules);
                }
                if !contributes.keybinds.is_empty() {
                    capabilities.push(Capability::Keybinds);
                }
            }
        }
        capabilities.sort();
        capabilities.dedup();
        capabilities
    }

    /// Read `<dir>/theme.ron` or `<dir>/plugin.ron`.
    pub fn load(dir: &Path) -> Result<Self, ModelError> {
        let theme = dir.join(Kind::Theme.manifest_file());
        let plugin = dir.join(Kind::Plugin.manifest_file());
        match (theme.is_file(), plugin.is_file()) {
            (true, true) => Err(ModelError::AmbiguousManifest(dir.to_owned())),
            (true, false) => Ok(Self::Theme(Box::new(parse_file(&theme)?))),
            (false, true) => Ok(Self::Plugin(Box::new(parse_file(&plugin)?))),
            (false, false) => Err(ModelError::NoManifest(dir.to_owned())),
        }
    }

    pub fn to_ron(&self) -> Result<String, ron::Error> {
        let pretty = ron::ser::PrettyConfig::default().struct_names(true);
        match self {
            Self::Theme(theme) => ron::ser::to_string_pretty(theme.as_ref(), pretty),
            Self::Plugin(plugin) => ron::ser::to_string_pretty(plugin.as_ref(), pretty),
        }
    }
}

pub(crate) fn ron_options() -> ron::Options {
    ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
}

pub fn parse_str<T: DeserializeOwned>(path: &str, text: &str) -> Result<T, ModelError> {
    ron_options()
        .from_str(text)
        .map_err(|error| ModelError::Parse {
            path: path.to_owned(),
            line: error.span.start.line,
            column: error.span.start.col,
            message: error.code.to_string(),
        })
}

fn parse_file<T: DeserializeOwned>(path: &Path) -> Result<T, ModelError> {
    let text = std::fs::read_to_string(path).map_err(ModelError::io(path))?;
    parse_str(&path.display().to_string(), &text)
}

pub(crate) fn api_v1() -> u32 {
    crate::API_VERSION
}
