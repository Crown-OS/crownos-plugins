//! Results arriving from worker threads, the prompt thread and file watchers.

use crownos_config::schema::{Decision, Plugins};
use crownplugin_proto::{LogLevel, Part};
use crownplugin_view::UiEvent;
use mlua::{IntoLuaMulti, Lua, MultiValue, Value};

use super::Daemon;

#[derive(Debug)]
pub enum Completion {
    Http { status: u16, body: String },
    Exec { code: i32, stdout: String, stderr: String },
    Text(String),
    Entries(Vec<String>),
    Done,
    Error(String),
}

impl Completion {
    fn into_args(self, lua: &Lua) -> mlua::Result<MultiValue> {
        let table = |pairs: Vec<(&str, Value)>| -> mlua::Result<Value> {
            let table = lua.create_table()?;
            for (key, value) in pairs {
                table.set(key, value)?;
            }
            Ok(Value::Table(table))
        };
        let string = |text: &str| -> mlua::Result<Value> { Ok(Value::String(lua.create_string(text)?)) };
        let value = match self {
            Self::Error(message) => return (Value::Nil, message).into_lua_multi(lua),
            Self::Http { status, body } => table(vec![
                ("status", Value::Integer(i64::from(status))),
                ("body", string(&body)?),
            ])?,
            Self::Exec { code, stdout, stderr } => table(vec![
                ("code", Value::Integer(i64::from(code))),
                ("stdout", string(&stdout)?),
                ("stderr", string(&stderr)?),
            ])?,
            Self::Text(text) => string(&text)?,
            Self::Entries(entries) => Value::Table(lua.create_sequence_from(entries)?),
            Self::Done => Value::Boolean(true),
        };
        (value, Value::Nil).into_lua_multi(lua)
    }
}

#[derive(Debug)]
pub enum Message {
    Completed { entity: String, request: u32, completion: Completion },
    Answered { entity: String, grant: String, decision: Option<Decision> },
    PluginsEdited(Plugins),
    ThemeChanged,
    SettingsEdited(String),
    DevChanged(String),
    UiEvent { plugin: String, widget: String, part: Part, event: UiEvent },
}

impl Daemon {
    pub fn handle(&mut self, message: Message) {
        match message {
            Message::Completed { entity, request, completion } => {
                let Some(running) = self.running.get_mut(&entity) else {
                    return;
                };
                let outcome = match completion.into_args(running.vm.lua()) {
                    Ok(args) => running.vm.complete(request, args),
                    Err(error) => crate::vm::Outcome::Failed(error.to_string()),
                };
                self.after_call(&entity, outcome);
            }
            Message::Answered { entity, grant, decision } => self.answered(&entity, &grant, decision),
            Message::PluginsEdited(plugins) => {
                if plugins != *self.store.plugins() {
                    self.store.replace(plugins);
                    self.reconcile();
                }
            }
            Message::ThemeChanged => self.theme_changed(),
            Message::SettingsEdited(id) => self.settings_edited(&id),
            Message::DevChanged(id) => self.debounce_reload(id),
            Message::UiEvent { plugin, widget, part, event } => {
                if let Some(running) = self.running.get_mut(&plugin) {
                    let outcome = running.vm.ui_event(&widget, part, &event);
                    self.after_call(&plugin, outcome);
                }
            }
        }
    }

    fn debounce_reload(&mut self, id: String) {
        if !self.dev_pending.insert(id.clone()) {
            return;
        }
        let timer = calloop::timer::Timer::from_duration(std::time::Duration::from_millis(250));
        let inserted = self.handle.insert_source(timer, move |_, _, daemon: &mut Daemon| {
            daemon.dev_pending.remove(&id);
            daemon.reload(&id);
            calloop::timer::TimeoutAction::Drop
        });
        if let Err(error) = inserted {
            tracing::warn!("cannot schedule a reload: {error}");
        }
    }

    fn theme_changed(&mut self) {
        self.outbox.lock = true;
        self.schedule_flush();
        let appearance: crownos_config::Appearance = crownos_config::load("appearance");
        let mode_changed = appearance.dark_mode != self.dark_mode;
        self.dark_mode = appearance.dark_mode;
        let tokens = self.tokens();
        let ids: Vec<String> = self.running.keys().cloned().collect();
        for id in ids {
            let Some(running) = self.running.get_mut(&id) else {
                continue;
            };
            {
                let mut host = running.vm.host();
                host.dark_mode = appearance.dark_mode;
                host.tokens = tokens.clone();
            }
            let outcome = running.vm.emit("theme", appearance.dark_mode);
            self.after_call(&id, outcome);
        }
        if mode_changed && let Some(theme) = self.store.active_theme().map(str::to_owned) {
            self.reapply_mode_dependent(&theme);
        }
    }

    /// Wallpaper and toolkit colors follow light and dark mode.
    fn reapply_mode_dependent(&mut self, id: &str) {
        let Some(entry) = self.registry.get(id) else {
            return;
        };
        let crownplugin_model::Manifest::Theme(theme) = &entry.package.manifest else {
            return;
        };
        let package = entry.package.clone();
        let theme = theme.clone();
        let changes = match crate::changes::theme_changes(&package, &theme) {
            Ok(changes) => changes
                .into_iter()
                .filter(|change| match change {
                    crate::journal::Change::Set { key, .. } => key == "wallpaper",
                    crate::journal::Change::File { .. } => true,
                    _ => false,
                })
                .collect(),
            Err(error) => {
                self.log(id, LogLevel::Error, format!("{error:#}"));
                return;
            }
        };
        if let Err(error) = self.apply_changes(id, changes) {
            self.log(id, LogLevel::Error, format!("{error:#}"));
        }
    }

    fn settings_edited(&mut self, id: &str) {
        let Some(entry) = self.registry.get(id) else {
            return;
        };
        let crownplugin_model::Manifest::Plugin(plugin) = &entry.package.manifest else {
            return;
        };
        let Some(section) = &plugin.contributes.settings else {
            return;
        };
        let values = section.resolve(&crate::store::load_settings(&plugin.id));
        let Some(running) = self.running.get_mut(id) else {
            return;
        };
        running.vm.host().settings = values;
        let outcome = running.vm.emit("settings", ());
        self.after_call(id, outcome);
        self.outbox.entities = true;
        self.schedule_flush();
    }
}
