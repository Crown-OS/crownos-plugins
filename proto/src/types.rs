use crownos_config::schema::Decision;
use crownplugin_model::{Kind, LauncherSize, Level, LockSlotPosition, SettingsSection};
use crownplugin_view::Tree;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Host {
    Bar,
    Lock,
    Settings,
    Cli,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Part {
    /// The widget itself: a bar pill or a lock-screen slot.
    Main,
    /// A bar widget's panel.
    Popup,
}

/// `plugin:<plugin>/<widget>` plus which part of it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SurfaceKey {
    pub plugin: String,
    pub widget: String,
    pub part: Part,
}

impl SurfaceKey {
    /// The entry this widget is listed under in `bar.ron`.
    pub fn bar_entry(&self) -> String {
        format!("plugin:{}/{}", self.plugin, self.widget)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteSurface {
    pub plugin: String,
    pub widget: String,
    pub main: Option<Tree>,
    pub popup: Option<Tree>,
    /// Lock-screen placement; ignored by the bar.
    pub lock_slot: Option<LockSlotPosition>,
}

impl RemoteSurface {
    pub fn key(&self, part: Part) -> SurfaceKey {
        SurfaceKey {
            plugin: self.plugin.clone(),
            widget: self.widget.clone(),
            part,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub revision: u64,
    pub surfaces: Vec<RemoteSurface>,
}

/// One plugin tile on the launcher's home grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LauncherSurface {
    pub plugin: String,
    pub widget: String,
    pub size: LauncherSize,
    pub tree: Tree,
}

impl LauncherSurface {
    pub fn key(&self) -> SurfaceKey {
        SurfaceKey {
            plugin: self.plugin.clone(),
            widget: self.widget.clone(),
            part: Part::Main,
        }
    }

    /// The entry this tile is listed under in `launcher.ron`.
    pub fn entry(&self) -> String {
        format!("plugin:{}/{}", self.plugin, self.widget)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LauncherSnapshot {
    pub revision: u64,
    pub tiles: Vec<LauncherSurface>,
}

/// One result a search provider offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    /// Handed back to the plugin's `on_activate` when the user picks it.
    pub id: String,
    pub title: String,
    pub subtitle: String,
    /// A Lucide icon name.
    pub icon: Option<String>,
}

/// What one provider answered for one query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResults {
    pub plugin: String,
    pub provider: String,
    /// The heading the hits are grouped under.
    pub name: String,
    /// The query as the launcher sent it, keyword included.
    pub query: String,
    pub hits: Vec<SearchHit>,
}

/// Names one hit for `activate`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HitKey {
    pub plugin: String,
    pub provider: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// A package folder or a `.crownpkg` file.
    Path(String),
    Git { url: String, rev: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityStatus {
    Inactive,
    Running,
    /// Crashed too often and stopped for this session.
    Quarantined(String),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityInfo {
    pub key: String,
    pub summary: String,
    pub level: Level,
    pub decision: Option<Decision>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityInfo {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub dir: String,
    /// Shipped with the system; cannot be uninstalled.
    pub system: bool,
    /// Enabled plugin, or the active theme.
    pub active: bool,
    pub dev: bool,
    pub status: EntityStatus,
    pub capabilities: Vec<CapabilityInfo>,
    pub settings: Option<SettingsSection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLine {
    pub id: String,
    pub level: LogLevel,
    /// Unix milliseconds.
    pub at: u64,
    pub message: String,
}
