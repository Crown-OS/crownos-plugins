//! crownplugind's link to the compositor over `crownos_plugin_host_v1`.

use std::collections::HashMap;
use std::path::Path;
use std::io::{Seek, SeekFrom, Write};
use std::os::fd::{AsFd, OwnedFd};

use anyhow::{Context, Result};
use calloop::LoopHandle;
use calloop_wayland_source::WaylandSource;
use crownos_protocols::plugin_host::v1::client::crownos_plugin_host_v1::{
    self, ActionResult, CrownosPluginHostV1, Watch,
};
use crownplugin_proto::LogLevel;
use crownos_config::schema::LockSlotPosition;
use crownplugin_view::Tree;
use lockscene::LockScene;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};

use crate::daemon::Daemon;
use crate::lock_assets::LockAssets;
use crate::registry::Registry;
use crate::surfaces::Surfaces;


pub struct Link {
    host: CrownosPluginHostV1,
    connection: Connection,
    serial: u32,
    watching: (bool, bool),
    pub locked: bool,
    sent: HashMap<String, u32>,
    /// serial -> (plugin, action), until `action_done` answers it.
    actions: HashMap<u32, (String, String)>,
    frame: Option<u64>,
    assets: LockAssets,
}

/// The receiver shares this fd's offset, so it is rewound to the start.
fn memfd(bytes: &[u8]) -> Result<OwnedFd> {
    let fd = rustix::fs::memfd_create(c"crownplugind", rustix::fs::MemfdFlags::CLOEXEC)?;
    let mut file = std::fs::File::from(fd);
    file.write_all(bytes)?;
    file.seek(SeekFrom::Start(0))?;
    Ok(file.into())
}

impl Link {
    pub fn connect(handle: &LoopHandle<'static, Daemon>) -> Result<Self> {
        let connection = Connection::connect_to_env().context("no Wayland display")?;
        let (globals, queue) = registry_queue_init::<Daemon>(&connection)?;
        let qh = queue.handle();
        let host: CrownosPluginHostV1 = globals
            .bind(&qh, 1..=1, ())
            .context("the compositor does not offer crownos_plugin_host_v1 to this session")?;
        WaylandSource::new(connection.clone(), queue)
            .insert(handle.clone())
            .map_err(|error| anyhow::anyhow!("watching the Wayland socket: {}", error.error))?;
        Ok(Self {
            host,
            connection,
            serial: 0,
            watching: (false, false),
            locked: false,
            sent: HashMap::new(),
            actions: HashMap::new(),
            frame: None,
            assets: LockAssets::default(),
        })
    }

    fn flush(&self) {
        if let Err(error) = self.connection.flush() {
            tracing::warn!("compositor link: {error}");
        }
    }

    pub fn run_action(&mut self, plugin: &str, action: &str) {
        self.serial = self.serial.wrapping_add(1);
        self.actions.insert(self.serial, (plugin.to_owned(), action.to_owned()));
        self.host.run_action(self.serial, action.to_owned());
        self.flush();
    }

    pub fn watch(&mut self, windows: bool, workspaces: bool) {
        if self.watching == (windows, workspaces) {
            return;
        }
        self.watching = (windows, workspaces);
        let mut events = Watch::empty();
        events.set(Watch::Windows, windows);
        events.set(Watch::Workspaces, workspaces);
        self.host.watch(events);
        self.flush();
    }

    fn put_assets(&mut self, wanted: &std::collections::BTreeSet<lockscene::AssetId>) -> Result<()> {
        for (id, bytes) in self.assets.missing(wanted) {
            let fd = memfd(&bytes)?;
            self.host.put_lock_asset(id.clone(), fd.as_fd(), bytes.len() as u32);
            self.assets.held(id);
        }
        Ok(())
    }

    /// Bring the compositor's lock scene up to date: the whole scene when
    /// its frame changed, otherwise only the slots whose trees did.
    pub fn push_lock(
        &mut self,
        surfaces: &Surfaces,
        registry: &Registry,
        theme_dir: Option<&Path>,
    ) -> Result<()> {
        let scene = self.assets.scene(surfaces, registry, theme_dir);
        lockscene::validate(scene.clone()).map_err(|error| anyhow::anyhow!("lock scene refused: {error}"))?;
        let frame = scene.without_slots().encode()?;
        let placements: Vec<(&str, LockSlotPosition, [f32; 2])> = scene
            .slots
            .iter()
            .map(|slot| (slot.id.as_str(), slot.position, slot.offset))
            .collect();
        let frame_hash = crate::util::hash::fnv1a(&[frame, format!("{placements:?}").into_bytes()].concat());
        if self.frame != Some(frame_hash) {
            let referenced = scene.referenced_assets();
            self.put_assets(&referenced)?;
            let bytes = scene.encode()?;
            let fd = memfd(&bytes)?;
            self.host.set_lock_scene(fd.as_fd(), bytes.len() as u32);
            self.assets.retain(&referenced);
            self.frame = Some(frame_hash);
            self.sent = scene.slots.iter().map(|slot| (slot.id.clone(), slot.tree.rev)).collect();
        } else {
            for slot in &scene.slots {
                if self.sent.get(&slot.id) == Some(&slot.tree.rev) {
                    continue;
                }
                let wanted = LockScene {
                    slots: vec![slot.clone()],
                    decoration: None,
                    ..scene.without_slots()
                }
                .referenced_assets();
                self.put_assets(&wanted)?;
                self.send_slot(&slot.id, &slot.tree)?;
                self.sent.insert(slot.id.clone(), slot.tree.rev);
            }
        }
        self.flush();
        Ok(())
    }

    fn send_slot(&self, id: &str, tree: &Tree) -> Result<()> {
        let bytes = crownos_ipc::postcard::to_stdvec(tree)?;
        let fd = memfd(&bytes)?;
        self.host.set_lock_slot(id.to_owned(), fd.as_fd(), bytes.len() as u32);
        Ok(())
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for Daemon {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wayland_client::protocol::wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<CrownosPluginHostV1, ()> for Daemon {
    fn event(
        daemon: &mut Self,
        _: &CrownosPluginHostV1,
        event: crownos_plugin_host_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use crownos_plugin_host_v1::Event;
        match event {
            Event::ActionDone { serial, result } => {
                let Some((plugin, action)) = daemon
                    .compositor
                    .as_mut()
                    .and_then(|link| link.actions.remove(&serial))
                else {
                    return;
                };
                let outcome = match result {
                    WEnum::Value(ActionResult::Done) => return,
                    WEnum::Value(ActionResult::Invalid) => "is not a compositor action",
                    WEnum::Value(ActionResult::Refused) => "was refused by the compositor",
                    _ => "failed",
                };
                daemon.log(&plugin, LogLevel::Warn, format!("`{action}` {outcome}"));
            }
            Event::PluginAction { plugin, name } => {
                if let Some(running) = daemon.running.get_mut(&plugin) {
                    let outcome = running.vm.action(&name);
                    daemon.after_call(&plugin, outcome);
                } else {
                    daemon.log(&plugin, LogLevel::Warn, format!("`{name}` fired but the plugin is not running"));
                }
            }
            Event::LockState { locked } => {
                if let Some(link) = daemon.compositor.as_mut() {
                    link.locked = locked != 0;
                }
                daemon.broadcast("lock_state", locked != 0);
            }
            Event::WorkspaceChanged { output, index } => daemon.broadcast("workspace_changed", (output, index)),
            Event::WindowOpened { id, app_id, title, workspace } => {
                daemon.broadcast("window_opened", (id, app_id, title, workspace));
            }
            Event::WindowChanged { id, app_id, title } => daemon.broadcast("window_changed", (id, app_id, title)),
            Event::WindowClosed { id } => daemon.broadcast("window_closed", id),
            Event::WindowFocused { id } => daemon.broadcast("window_focused", id),
            _ => {}
        }
    }
}

impl Daemon {
    pub fn connect_compositor(&mut self) {
        match Link::connect(&self.handle) {
            Ok(link) => {
                self.compositor = Some(link);
                self.update_watch();
                self.outbox.lock = true;
                self.schedule_flush();
            }
            Err(error) => tracing::info!("running without the compositor link: {error:#}"),
        }
    }

    pub fn broadcast<A: mlua::IntoLuaMulti + Clone>(&mut self, event: &str, args: A) {
        let ids: Vec<String> = self
            .running
            .iter()
            .filter(|(_, running)| running.vm.host().listeners.contains_key(event))
            .map(|(id, _)| id.clone())
            .collect();
        let private = event.starts_with("window_");
        for id in ids {
            if private
                && !matches!(
                    self.permit(&id, &crownplugin_model::Capability::WindowFeed),
                    crate::daemon::Permit::Granted
                )
            {
                continue;
            }
            if let Some(running) = self.running.get_mut(&id) {
                let outcome = running.vm.emit(event, args.clone());
                self.after_call(&id, outcome);
            }
        }
    }
}
