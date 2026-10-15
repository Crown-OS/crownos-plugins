//! The daemon's state and everything that changes it. One thread, one
//! calloop loop: IPC requests, timers, worker results and config edits all
//! arrive as calls on [`Daemon`].

mod commands;
mod flush;
mod install;
mod lifecycle;
mod messages;
mod rpc;

use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use calloop::channel::Sender;
use calloop::{LoopHandle, RegistrationToken};
use crownos_ipc::adapter::calloop::CalloopService;
use crownplugin_proto::LogLine;

use crate::compositor::Link;
use crate::registry::Registry;
use crate::store::Store;
use crate::surfaces::Surfaces;
use crate::vm::{Command, Vm};
use crate::watch::Watches;

pub use commands::Permit;
pub use messages::{Completion, Message};

/// A running VM and the loop sources it owns.
pub struct Running {
    pub vm: Vm,
    pub timers: HashMap<u32, RegistrationToken>,
    pub _settings_watch: Option<crownos_config::Subscription>,
}

/// A command waiting for the user to decide on a capability.
pub struct Parked {
    pub entity: String,
    pub grant: String,
    pub command: Command,
}

#[derive(Default)]
pub struct Outbox {
    pub bar: bool,
    pub lock: bool,
    pub entities: bool,
    pub logs: Vec<LogLine>,
    pub scheduled: bool,
}

pub struct Daemon {
    pub handle: LoopHandle<'static, Daemon>,
    pub sender: Sender<Message>,
    pub registry: Registry,
    pub store: Store,
    pub running: HashMap<String, Running>,
    pub surfaces: Surfaces,
    pub outbox: Outbox,
    pub parked: Vec<Parked>,
    pub prompting: BTreeSet<(String, String)>,
    pub dev_pending: BTreeSet<String>,
    pub ipc: Option<Rc<CalloopService>>,
    pub compositor: Option<Link>,
    pub watches: Watches,
    pub entities_revision: u64,
    pub dark_mode: bool,
}

impl Daemon {
    pub fn new(handle: LoopHandle<'static, Daemon>, sender: Sender<Message>) -> Self {
        let mut registry = Registry::default();
        registry.rescan();
        Self {
            handle,
            watches: Watches::start(sender.clone()),
            sender,
            registry,
            store: Store::load(),
            running: HashMap::new(),
            surfaces: Surfaces::default(),
            outbox: Outbox::default(),
            parked: Vec::new(),
            prompting: BTreeSet::new(),
            dev_pending: BTreeSet::new(),
            ipc: None,
            compositor: None,
            entities_revision: 0,
            dark_mode: crownos_config::load::<crownos_config::Appearance>("appearance").dark_mode,
        }
    }
}
