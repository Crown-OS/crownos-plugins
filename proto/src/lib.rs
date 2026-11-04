//! The wire contract of `crownplugind`.
//!
//! The daemon serves; the bar, Settings, the lock screen glue and the
//! `crownplugin` CLI connect. Every tree a host sees arrives as a whole,
//! versioned snapshot, so a host that missed an event under backpressure just
//! calls `attach` again.

mod types;

pub use types::{
    CapabilityInfo, EntityInfo, EntityStatus, HitKey, Host, LauncherSnapshot, LauncherSurface,
    LogLevel, LogLine, Part, RemoteSurface, SearchHit, SearchResults, Snapshot, Source,
    SurfaceKey,
};

pub use crownplugin_model::settings::{SettingValue, SettingsSection};
pub use crownplugin_model::{Kind, LauncherSize, Level, LockSlotPosition};
pub use crownos_config::schema::Decision;
pub use crownplugin_view::{Node, NodeId, Tree, UiEvent};

crownos_ipc::protocol! {
    pub mod plugind: "crownplugind" {
        /// Everything this host draws right now; also the resync after a gap.
        fn attach(host: Host) -> Snapshot;
        /// The user acted on a node of a plugin surface.
        notify ui_event(surface: SurfaceKey, rev: u32, event: UiEvent);
        /// A plugin's bar popup opened or closed.
        notify popup(surface: SurfaceKey, open: bool);

        /// Every tile plugins draw on the launcher's home grid.
        fn attach_launcher() -> LauncherSnapshot;
        /// The launcher opened or closed; plugins refresh their tiles on open.
        notify launcher_shown(shown: bool);
        /// Ask every search provider about `query`; answers arrive as
        /// `SearchAnswered`, possibly long after, and only for this query.
        notify search(query: String);
        /// The user picked a provider's hit.
        notify activate(hit: HitKey);

        fn list() -> Vec<EntityInfo>;
        /// Install from a folder, a `.crownpkg` file or a git URL; returns the id.
        fn install(source: Source) -> String;
        fn uninstall(id: String);
        fn set_enabled(id: String, enabled: bool);
        /// Activate a theme, or go back to the stock look with `None`.
        fn set_theme(id: Option<String>);
        /// Record or forget the user's decision for one capability.
        fn set_grant(id: String, capability: String, decision: Option<Decision>);
        fn settings(id: String) -> Vec<(String, SettingValue)>;
        fn set_setting(id: String, key: String, value: SettingValue);
        fn logs(id: String) -> Vec<LogLine>;
        /// Load an unpacked folder in place and reload it whenever it changes.
        fn dev_load(path: String) -> String;
        fn reload(id: String);

        event BarChanged { snapshot: Snapshot };
        event LockChanged { snapshot: Snapshot };
        event LauncherChanged { snapshot: LauncherSnapshot };
        event SearchAnswered { results: SearchResults };
        event EntitiesChanged { revision: u64 };
        event Logged { line: LogLine };
    }
}
