//! What a CrownOS theme or plugin *is*: its manifest, the capabilities it
//! asks for, where it lives on disk and whether it is well formed.

#[cfg(feature = "archive")]
pub mod archive;
pub mod capability;
pub mod config_access;
mod error;
pub mod id;
pub mod manifest;
pub mod package;
pub mod settings;
pub mod validate;

pub use capability::{Capability, Feed, Level, UserDir};
pub use config_access::{ConfigAccess, config_access};
pub use error::ModelError;
pub use id::EntityId;
pub use manifest::{
    BarSlot, BarWidgetDecl, ConfigPatch, Contributes, KeybindDecl, Kind, Limits, LockSlotDecl,
    LockSlotPosition, LockTheme, Manifest, PluginManifest, RonText, ThemeAssets, ThemeManifest,
    Tokens, Wallpaper,
};
pub use package::Package;
pub use settings::{Field, SelectOption, SettingValue, SettingsSection};
pub use validate::{Issue, Report, validate};

/// The manifest format this build understands.
pub const API_VERSION: u32 = 1;
