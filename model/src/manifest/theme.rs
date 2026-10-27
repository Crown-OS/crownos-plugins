use std::collections::BTreeMap;

use crownos_config::schema::theme::{AccentGradient, Font, Radii};
use crownos_config::schema::{ClockStyle, LockBackground, LockLayout, LockTransitions, Placement};
use serde::{Deserialize, Serialize};

use super::{ConfigPatch, api_v1};
use crate::{Capability, EntityId};

/// A whole look: tokens, config, wallpaper, lock screen and system assets.
/// Exactly one theme is active at a time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "Theme")]
pub struct ThemeManifest {
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
    #[serde(default)]
    pub tokens: Tokens,
    #[serde(default)]
    pub config: ConfigPatch,
    #[serde(default)]
    pub wallpaper: Option<Wallpaper>,
    #[serde(default)]
    pub lock: Option<LockTheme>,
    #[serde(default)]
    pub assets: ThemeAssets,
    /// Optional Luau script for dynamic behaviour, e.g. a time-of-day wallpaper.
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tokens {
    pub light: BTreeMap<String, String>,
    pub dark: BTreeMap<String, String>,
    pub accent: Option<AccentGradient>,
    pub radii: Option<Radii>,
    pub font: Option<Font>,
}

/// Package-relative image paths; one may be left out to use the other in both modes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wallpaper {
    pub light: Option<String>,
    pub dark: Option<String>,
}

impl Wallpaper {
    pub fn for_mode(&self, dark_mode: bool) -> Option<&str> {
        let (preferred, other) = if dark_mode {
            (&self.dark, &self.light)
        } else {
            (&self.light, &self.dark)
        };
        preferred.as_deref().or(other.as_deref())
    }
}

/// Lock-screen defaults the theme brings; each one is written into the
/// `lock` section. An `Image` background is package-relative here and
/// rewritten to an absolute path when the theme is applied.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LockTheme {
    pub background: Option<LockBackground>,
    pub clock_style: Option<ClockStyle>,
    pub time_format: Option<String>,
    pub date_format: Option<String>,
    /// A font family the theme installs system-wide through `assets.system`.
    pub font: Option<String>,
    pub layout: Option<LockLayout>,
    pub dim: Option<f32>,
    pub blur: Option<f32>,
    pub auth_field: Option<Placement>,
    pub transitions: Option<LockTransitions>,
    pub show_user: Option<bool>,
}

/// Package-relative directories.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeAssets {
    /// A freedesktop icon theme directory (holds `index.theme`).
    pub icons: Option<String>,
    /// An XCursor theme directory (holds `index.theme` and `cursors/`).
    pub cursors: Option<String>,
    /// Font files or directories installed for the user.
    pub fonts: Vec<String>,
    /// A freedesktop sound theme directory.
    pub sounds: Option<String>,
    /// Fonts and cursors the compositor needs, installed system-wide; the
    /// compositor cannot read the user's home.
    pub system: Option<String>,
    /// Generate GTK 3/4 colors from the tokens.
    pub gtk: bool,
    /// Generate a Qt color scheme from the tokens.
    pub qt: bool,
}
