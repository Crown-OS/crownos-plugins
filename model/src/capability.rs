//! What a theme or plugin may do, and how much the user has to be asked.

use std::borrow::Cow;
use std::fmt;

use serde::{Deserialize, Serialize};

/// How a capability is granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Level {
    /// Granted on install; nothing to ask.
    Safe,
    /// Asked the first time it is used; the answer is remembered.
    Sensitive,
    /// Asked with a warning the first time it is used; remembered and revocable.
    Dangerous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum UserDir {
    Home,
    Documents,
    Downloads,
    Music,
    Pictures,
    Videos,
}

impl UserDir {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Documents => "documents",
            Self::Downloads => "downloads",
            Self::Music => "music",
            Self::Pictures => "pictures",
            Self::Videos => "videos",
        }
    }
}

/// Live data a plugin can subscribe to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Feed {
    Time,
    /// CPU, memory, uptime.
    System,
    /// Workspace switches, without window titles.
    Workspaces,
    /// Battery and power source.
    Power,
}

impl Feed {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::System => "system",
            Self::Workspaces => "workspaces",
            Self::Power => "power",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Capability {
    OwnConfig,
    BarWidget,
    LockSlot,
    SettingsSection,
    ReadTheme,
    Feed(Feed),
    /// Write look-only config: `appearance`, `theme`, `bar`, `lock`.
    ThemeVisual,

    Network { hosts: Vec<String> },
    FsRead(UserDir),
    Notify,
    /// Write a behavioural config section that has no capability of its own.
    Config { section: String },
    /// Window app ids and titles.
    WindowFeed,
    WindowRules,
    CompositorActions,

    Exec { programs: Vec<String> },
    FsWrite(UserDir),
    Keybinds,
    PowerConfig,
    DisplayConfig,
    StartupConfig,
}

impl Capability {
    pub fn level(&self) -> Level {
        match self {
            Self::OwnConfig
            | Self::BarWidget
            | Self::LockSlot
            | Self::SettingsSection
            | Self::ReadTheme
            | Self::Feed(_)
            | Self::ThemeVisual => Level::Safe,
            Self::Network { .. }
            | Self::FsRead(_)
            | Self::Notify
            | Self::Config { .. }
            | Self::WindowFeed
            | Self::WindowRules
            | Self::CompositorActions => Level::Sensitive,
            Self::Exec { .. }
            | Self::FsWrite(_)
            | Self::Keybinds
            | Self::PowerConfig
            | Self::DisplayConfig
            | Self::StartupConfig => Level::Dangerous,
        }
    }

    /// Stable name the user's decision is stored under in `plugins.ron`.
    pub fn grant_key(&self) -> Cow<'static, str> {
        match self {
            Self::OwnConfig => "own_config".into(),
            Self::BarWidget => "bar_widget".into(),
            Self::LockSlot => "lock_slot".into(),
            Self::SettingsSection => "settings_section".into(),
            Self::ReadTheme => "read_theme".into(),
            Self::Feed(feed) => format!("feed.{}", feed.as_str()).into(),
            Self::ThemeVisual => "theme_visual".into(),
            Self::Network { .. } => "network".into(),
            Self::FsRead(dir) => format!("fs_read.{}", dir.as_str()).into(),
            Self::Notify => "notify".into(),
            Self::Config { section } => format!("config.{section}").into(),
            Self::WindowFeed => "window_feed".into(),
            Self::WindowRules => "window_rules".into(),
            Self::CompositorActions => "compositor_actions".into(),
            Self::Exec { .. } => "exec".into(),
            Self::FsWrite(dir) => format!("fs_write.{}", dir.as_str()).into(),
            Self::Keybinds => "keybinds".into(),
            Self::PowerConfig => "power_config".into(),
            Self::DisplayConfig => "display_config".into(),
            Self::StartupConfig => "startup_config".into(),
        }
    }

    /// What the user is asked to allow.
    pub fn summary(&self) -> String {
        match self {
            Self::OwnConfig => "Store its own settings".into(),
            Self::BarWidget => "Add widgets to the bar".into(),
            Self::LockSlot => "Show content on the lock screen".into(),
            Self::SettingsSection => "Add a page to Settings".into(),
            Self::ReadTheme => "Read the system theme".into(),
            Self::Feed(feed) => format!("Read {} information", feed.as_str()),
            Self::ThemeVisual => "Change the look of the desktop".into(),
            Self::Network { hosts } => format!("Connect to {}", hosts.join(", ")),
            Self::FsRead(dir) => format!("Read files in your {} folder", dir.as_str()),
            Self::Notify => "Show notifications".into(),
            Self::Config { section } => format!("Change your {section} settings"),
            Self::WindowFeed => "See which apps are open and their window titles".into(),
            Self::WindowRules => "Decide where and how app windows open".into(),
            Self::CompositorActions => "Control windows and workspaces".into(),
            Self::Exec { programs } => format!("Run programs: {}", programs.join(", ")),
            Self::FsWrite(dir) => format!("Change files in your {} folder", dir.as_str()),
            Self::Keybinds => "Add global keyboard shortcuts".into(),
            Self::PowerConfig => "Change power and sleep settings".into(),
            Self::DisplayConfig => "Change display settings".into(),
            Self::StartupConfig => "Run programs when you log in".into(),
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.grant_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_the_three_tiers() {
        assert_eq!(Capability::BarWidget.level(), Level::Safe);
        assert_eq!(Capability::Feed(Feed::System).level(), Level::Safe);
        assert_eq!(Capability::Notify.level(), Level::Sensitive);
        assert_eq!(
            Capability::Network { hosts: vec!["x.org".into()] }.level(),
            Level::Sensitive
        );
        assert_eq!(Capability::Exec { programs: vec![] }.level(), Level::Dangerous);
        assert_eq!(Capability::Keybinds.level(), Level::Dangerous);
    }

    #[test]
    fn grant_keys_are_distinct_per_scope() {
        assert_eq!(Capability::FsRead(UserDir::Music).grant_key(), "fs_read.music");
        assert_ne!(
            Capability::FsRead(UserDir::Music).grant_key(),
            Capability::FsWrite(UserDir::Music).grant_key()
        );
    }

    #[test]
    fn manifests_spell_capabilities_naturally() {
        let caps: Vec<Capability> =
            ron::from_str(r#"[Network(hosts: ["api.example.com"]), FsRead(Music), Notify, Feed(System)]"#)
                .expect("parses");
        assert_eq!(caps[1], Capability::FsRead(UserDir::Music));
    }
}
