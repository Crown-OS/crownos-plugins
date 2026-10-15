//! Carrying out what scripts ask for, after checking they may.

use std::time::Duration;

use calloop::timer::{TimeoutAction, Timer};
use crownos_config::schema::Decision;
use crownplugin_model::{Capability, ConfigAccess, Level, Manifest, config_access};
use crownplugin_proto::LogLevel;

use super::{Completion, Daemon, Parked};
use crate::journal::Change;
use crate::surfaces::Changed;
use crate::vm::{Command, Method};
use crate::{store, workers};

pub enum Permit {
    Granted,
    Denied,
    Asked,
}

/// Programs and actions no plugin may reach through the compositor.
const REFUSED_ACTIONS: [&str; 3] = ["quit", "switch-vt", "spawn"];

impl Daemon {
    fn declared(&self, id: &str, matches: impl Fn(&Capability) -> bool) -> Option<Capability> {
        self.registry
            .get(id)?
            .package
            .manifest
            .capabilities()
            .into_iter()
            .find(|capability| matches(capability))
    }

    /// Whether `id` may use `capability` now, asking the user the first time.
    pub fn permit(&mut self, id: &str, capability: &Capability) -> Permit {
        if capability.level() == Level::Safe {
            return Permit::Granted;
        }
        let grant = capability.grant_key().into_owned();
        match self.store.decision(id, &grant) {
            Some(Decision::Granted) => Permit::Granted,
            Some(Decision::Denied) => Permit::Denied,
            None => {
                if self.prompting.insert((id.to_owned(), grant)) {
                    let name = self
                        .registry
                        .get(id)
                        .map(|entry| entry.package.manifest.name().to_owned())
                        .unwrap_or_else(|| id.to_owned());
                    crate::prompts::ask(self.sender.clone(), id, &name, capability);
                }
                Permit::Asked
            }
        }
    }

    fn gated(&mut self, id: &str, capability: Option<Capability>, command: Command) -> Option<Command> {
        let Some(capability) = capability else {
            self.fail(id, &command, "the manifest does not declare this capability");
            return None;
        };
        match self.permit(id, &capability) {
            Permit::Granted => Some(command),
            Permit::Denied => {
                self.fail(id, &command, "permission denied");
                None
            }
            Permit::Asked => {
                self.parked.push(Parked {
                    entity: id.to_owned(),
                    grant: capability.grant_key().into_owned(),
                    command,
                });
                None
            }
        }
    }

    fn fail(&mut self, id: &str, command: &Command, reason: &str) {
        let request = match command {
            Command::Http { request, .. }
            | Command::Exec { request, .. }
            | Command::ReadFile { request, .. }
            | Command::ListDir { request, .. }
            | Command::WriteFile { request, .. } => Some(*request),
            _ => None,
        };
        match request {
            Some(request) => workers::complete(&self.sender, id, request, Completion::Error(reason.to_owned())),
            None => self.log(id, LogLevel::Warn, format!("{reason}: {command:?}")),
        }
    }

    pub fn run_command(&mut self, id: &str, command: Command) {
        match command {
            Command::Render { widget, part, tree } => self.render(id, &widget, part, tree),
            Command::LockDecoration(tree) => {
                if self.store.active_theme() == Some(id) && self.surfaces.set_lock_layout(tree) {
                    self.outbox.lock = true;
                    self.schedule_flush();
                }
            }
            Command::Timer { id: timer, every, repeat } => self.start_timer(id, timer, every, repeat),
            Command::CancelTimer(timer) => {
                if let Some(token) = self.running.get_mut(id).and_then(|running| running.timers.remove(&timer)) {
                    self.handle.remove(token);
                }
            }
            Command::Http { .. } => {
                let capability = self.declared(id, |c| matches!(c, Capability::Network { .. }));
                let hosts = match &capability {
                    Some(Capability::Network { hosts }) => hosts.clone(),
                    _ => Vec::new(),
                };
                if let Some(Command::Http { request, method, url, body }) = self.gated(id, capability, command) {
                    match workers::host_allowed(&url, &hosts) {
                        true => workers::http(self.sender.clone(), id, request, method == Method::Post, url, body),
                        false => workers::complete(
                            &self.sender,
                            id,
                            request,
                            Completion::Error(format!("{url} is not in the manifest's network hosts")),
                        ),
                    }
                }
            }
            Command::Exec { .. } => {
                let capability = self.declared(id, |c| matches!(c, Capability::Exec { .. }));
                let programs = match &capability {
                    Some(Capability::Exec { programs }) => programs.clone(),
                    _ => Vec::new(),
                };
                if let Some(Command::Exec { request, argv }) = self.gated(id, capability, command) {
                    if programs.iter().any(|program| Some(program) == argv.first()) {
                        workers::exec(self.sender.clone(), id, request, argv);
                    } else {
                        workers::complete(
                            &self.sender,
                            id,
                            request,
                            Completion::Error(format!("{} is not in the manifest's programs", argv[0])),
                        );
                    }
                }
            }
            Command::ReadFile { dir, .. } | Command::ListDir { dir, .. } => {
                let capability = self.declared(id, |c| *c == Capability::FsRead(dir));
                if let Some(command) = self.gated(id, capability, command) {
                    workers::files(&self.sender, id, command);
                }
            }
            Command::WriteFile { dir, .. } => {
                let capability = self.declared(id, |c| *c == Capability::FsWrite(dir));
                if let Some(command) = self.gated(id, capability, command) {
                    workers::files(&self.sender, id, command);
                }
            }
            Command::Notify { .. } => {
                let capability = self.declared(id, |c| *c == Capability::Notify);
                if let Some(Command::Notify { summary, body }) = self.gated(id, capability, command) {
                    let app = self
                        .registry
                        .get(id)
                        .map(|entry| entry.package.manifest.name().to_owned())
                        .unwrap_or_default();
                    workers::notify(app, summary, body);
                }
            }
            Command::RunAction(ref action) => {
                let verb = action.split_whitespace().next().unwrap_or_default();
                if REFUSED_ACTIONS.contains(&verb) {
                    self.log(id, LogLevel::Warn, format!("plugins may not run `{verb}`"));
                    return;
                }
                let capability = self.declared(id, |c| *c == Capability::CompositorActions);
                if let Some(Command::RunAction(action)) = self.gated(id, capability, command) {
                    match self.compositor.as_mut() {
                        Some(link) => link.run_action(id, &action),
                        None => self.log(id, LogLevel::Warn, "the compositor link is down".into()),
                    }
                }
            }
            Command::Watch(event) => {
                self.update_watch();
                let locked = self.compositor.as_ref().map(|link| link.locked);
                if event == "lock_state"
                    && let (Some(locked), Some(running)) = (locked, self.running.get_mut(id))
                {
                    let outcome = running.vm.emit("lock_state", locked);
                    self.after_call(id, outcome);
                }
            }
            Command::SetConfig { ref section, ref key, .. } => {
                let capability = match config_access(section, key) {
                    ConfigAccess::Visual => self.declared(id, |c| *c == Capability::ThemeVisual),
                    ConfigAccess::Needs(needed) => self.declared(id, |c| *c == needed),
                    ConfigAccess::Forbidden => None,
                };
                if let Some(Command::SetConfig { section, key, value }) = self.gated(id, capability, command) {
                    self.change(id, Change::Set { section, key, value });
                }
            }
            Command::SetWallpaper(path) => {
                let value = ron::to_string(&path.to_string_lossy()).unwrap_or_default();
                self.change(id, Change::Set { section: "appearance".into(), key: "wallpaper".into(), value });
            }
            Command::SetSetting { key, value } => self.set_setting_from_script(id, key, value),
            Command::Log(level, message) => self.log(id, level, message),
        }
    }

    fn change(&mut self, id: &str, change: Change) {
        if let Err(error) = self.apply_changes(id, vec![change]) {
            self.log(id, LogLevel::Error, format!("{error:#}"));
        }
    }

    fn render(&mut self, id: &str, widget: &str, part: crownplugin_proto::Part, tree: Option<crownplugin_view::Node>) {
        let lock = self.registry.get(id).and_then(|entry| match &entry.package.manifest {
            Manifest::Plugin(plugin) => plugin
                .contributes
                .lock
                .iter()
                .find(|slot| slot.id == widget)
                .map(|slot| slot.slot),
            Manifest::Theme(_) => None,
        });
        let entry_name = format!("plugin:{id}/{widget}");
        let lock = lock.map(|default| {
            let lock: crownos_config::Lock = crownos_config::load("lock");
            lock.slot_placement(&entry_name).map_or(default, |placement| placement.position)
        });
        match self.surfaces.render(id, widget, part, tree, lock) {
            Ok(Changed::Bar) => self.outbox.bar = true,
            Ok(Changed::Lock) => self.outbox.lock = true,
            Ok(Changed::Nothing) => return,
            Err(error) => {
                self.log(id, LogLevel::Error, error);
                return;
            }
        }
        self.schedule_flush();
    }

    fn start_timer(&mut self, id: &str, timer: u32, every: Duration, repeat: bool) {
        let entity = id.to_owned();
        let inserted = self.handle.insert_source(Timer::from_duration(every), move |_, _, daemon: &mut Daemon| {
            let Some(running) = daemon.running.get_mut(&entity) else {
                return TimeoutAction::Drop;
            };
            let Some(outcome) = running.vm.fire_timer(timer) else {
                running.timers.remove(&timer);
                return TimeoutAction::Drop;
            };
            daemon.after_call(&entity, outcome);
            let alive = daemon
                .running
                .get(&entity)
                .is_some_and(|running| running.timers.contains_key(&timer));
            if repeat && alive {
                TimeoutAction::ToDuration(every)
            } else {
                if let Some(running) = daemon.running.get_mut(&entity) {
                    running.timers.remove(&timer);
                    running.vm.host().timers.remove(&timer);
                }
                TimeoutAction::Drop
            }
        });
        match (inserted, self.running.get_mut(id)) {
            (Ok(token), Some(running)) => {
                running.timers.insert(timer, token);
            }
            (Ok(token), None) => self.handle.remove(token),
            (Err(error), _) => self.log(id, LogLevel::Error, format!("timer: {error}")),
        }
    }

    fn set_setting_from_script(&mut self, id: &str, key: String, value: crownplugin_model::SettingValue) {
        let Some(entry) = self.registry.get(id) else {
            return;
        };
        let package_id = entry.package.id().clone();
        let mut values = store::load_settings(&package_id);
        values.insert(key, value);
        store::save_settings(&package_id, &values);
        self.outbox.entities = true;
        self.schedule_flush();
    }

    /// The union of what every running script wants to hear from the compositor.
    pub fn update_watch(&mut self) {
        let (windows, workspaces) = self.running.values().fold((false, false), |(windows, workspaces), running| {
            let host = running.vm.host();
            let has = |name: &str| host.listeners.contains_key(name);
            (
                windows || ["window_opened", "window_changed", "window_closed", "window_focused"].iter().any(|name| has(name)),
                workspaces || has("workspace_changed"),
            )
        });
        if let Some(link) = self.compositor.as_mut() {
            link.watch(windows, workspaces);
        }
    }

    /// The user answered a prompt, or it could not be shown (`None`).
    pub fn answered(&mut self, id: &str, grant: &str, decision: Option<Decision>) {
        self.prompting.remove(&(id.to_owned(), grant.to_owned()));
        if let Some(decision) = decision {
            self.store.set_decision(id, grant, Some(decision));
            self.outbox.entities = true;
        }
        let (ready, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.parked)
            .into_iter()
            .partition(|parked| parked.entity == id && parked.grant == grant);
        self.parked = waiting;
        for parked in ready {
            match decision {
                Some(Decision::Granted) => self.run_command(id, parked.command),
                Some(Decision::Denied) => self.fail(id, &parked.command, "permission denied"),
                None => self.fail(id, &parked.command, "could not ask for permission"),
            }
        }
        self.schedule_flush();
    }
}
