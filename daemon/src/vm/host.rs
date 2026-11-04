use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::Duration;

use crownplugin_model::{LockSlotPosition, Manifest, Package, SettingValue, UserDir};
use crownplugin_proto::{LogLevel, Part, SearchHit};
use crownplugin_view::Node;
use mlua::Function;

use crate::feeds::system::Sampler;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// Something a script asked for; the daemon decides whether and how.
#[derive(Debug)]
pub enum Command {
    Render {
        widget: String,
        part: Part,
        tree: Option<Node>,
    },
    LockDecoration(Option<Node>),
    /// A launcher tile's tree.
    RenderTile {
        widget: String,
        tree: Option<Node>,
    },
    /// A search provider's hits for `query`.
    Answer {
        provider: String,
        query: String,
        hits: Vec<SearchHit>,
    },
    Timer {
        id: u32,
        every: Duration,
        repeat: bool,
    },
    CancelTimer(u32),
    Http {
        request: u32,
        method: Method,
        url: String,
        body: Option<String>,
    },
    Exec {
        request: u32,
        argv: Vec<String>,
    },
    ReadFile {
        request: u32,
        dir: UserDir,
        path: String,
    },
    ListDir {
        request: u32,
        dir: UserDir,
        path: String,
    },
    WriteFile {
        request: u32,
        dir: UserDir,
        path: String,
        text: String,
    },
    Notify {
        summary: String,
        body: String,
    },
    RunAction(String),
    /// The script started listening to a compositor event.
    Watch(String),
    SetConfig {
        section: String,
        key: String,
        value: String,
    },
    SetWallpaper(PathBuf),
    SetSetting {
        key: String,
        value: SettingValue,
    },
    Log(LogLevel, String),
}

/// Per-VM state the script-facing functions can reach.
pub struct Host {
    pub dir: PathBuf,
    pub is_theme: bool,
    pub commands: Vec<Command>,
    /// Handlers of the latest tree of each surface, indexed by node id.
    pub handlers: HashMap<(String, Part), Vec<Option<Function>>>,
    pub timers: HashMap<u32, Function>,
    pub requests: HashMap<u32, Function>,
    pub listeners: HashMap<String, Vec<Function>>,
    next_id: u32,
    pub bar_widgets: Vec<String>,
    pub lock_slots: Vec<(String, LockSlotPosition)>,
    pub launcher_tiles: Vec<String>,
    pub search_providers: Vec<String>,
    pub dark_mode: bool,
    pub tokens: BTreeMap<String, String>,
    pub settings: BTreeMap<String, SettingValue>,
    pub sampler: Sampler,
    pub modules: HashMap<String, mlua::Value>,
}

impl Host {
    pub fn new(package: &Package, is_theme: bool) -> Self {
        let (bar_widgets, lock_slots, launcher_tiles, search_providers) = match &package.manifest {
            Manifest::Plugin(plugin) => {
                let contributes = &plugin.contributes;
                (
                    contributes.bar.iter().map(|widget| widget.id.clone()).collect(),
                    contributes.lock.iter().map(|slot| (slot.id.clone(), slot.slot)).collect(),
                    contributes.launcher.iter().map(|tile| tile.id.clone()).collect(),
                    contributes.search.iter().map(|provider| provider.id.clone()).collect(),
                )
            }
            Manifest::Theme(_) => (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        };
        Self {
            dir: package.dir.clone(),
            is_theme,
            commands: Vec::new(),
            handlers: HashMap::new(),
            timers: HashMap::new(),
            requests: HashMap::new(),
            listeners: HashMap::new(),
            next_id: 1,
            bar_widgets,
            lock_slots,
            launcher_tiles,
            search_providers,
            dark_mode: true,
            tokens: BTreeMap::new(),
            settings: BTreeMap::new(),
            sampler: Sampler::default(),
            modules: HashMap::new(),
        }
    }

    pub fn next_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    /// Park `callback` until the daemon completes `request`.
    pub fn request(&mut self, callback: Option<Function>) -> u32 {
        let id = self.next_id();
        if let Some(callback) = callback {
            self.requests.insert(id, callback);
        }
        id
    }
}
