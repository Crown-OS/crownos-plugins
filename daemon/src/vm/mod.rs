//! One sandboxed Luau VM per running theme script or plugin.
//!
//! Script-facing functions never touch daemon state. They queue
//! [`Command`]s in the VM's [`Host`], which the daemon drains after every
//! callback, so a script can only ever do what the daemon then agrees to.

mod api;
mod convert;
mod host;

use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crownplugin_model::{Kind, Limits, Package};
use crownplugin_proto::Part;
use crownplugin_view::UiEvent;
use mlua::{Function, IntoLuaMulti, Lua, LuaOptions, StdLib, Table, Value, VmState};

pub use api::Grants;
pub use host::{Command, Host, Method};

/// What a script can read from its first line on.
#[derive(Debug, Clone, Default)]
pub struct Seed {
    pub dark_mode: bool,
    pub tokens: std::collections::BTreeMap<String, String>,
    pub settings: std::collections::BTreeMap<String, crownplugin_model::SettingValue>,
}

const STRIKES: usize = 3;
const STRIKE_WINDOW: Duration = Duration::from_secs(60);
const LOAD_BUDGET: Duration = Duration::from_millis(500);

pub struct Vm {
    lua: Lua,
    module: Option<Table>,
    budget: Duration,
    deadline: Rc<Cell<Option<Instant>>>,
    strikes: VecDeque<Instant>,
}

/// What became of a call into the script.
#[derive(Debug)]
pub enum Outcome {
    Ok,
    Failed(String),
    /// Too many failures in a short time; the VM must be stopped.
    Quarantine(String),
}

impl Vm {
    pub fn start(package: &Package, grants: &Grants, seed: Seed) -> anyhow::Result<(Self, Outcome)> {
        let manifest = &package.manifest;
        let limits = match manifest {
            crownplugin_model::Manifest::Plugin(plugin) => plugin.limits,
            crownplugin_model::Manifest::Theme(_) => Limits::default(),
        };
        let deadline: Rc<Cell<Option<Instant>>> = Rc::default();
        let lua = build(package, grants, limits, Rc::clone(&deadline), seed).map_err(lua_error)?;

        let mut vm = Self {
            lua,
            module: None,
            budget: Duration::from_millis(u64::from(limits.call_budget_ms)),
            deadline,
            strikes: VecDeque::new(),
        };
        let outcome = match manifest.script() {
            Some(script) => {
                let source = package.read_text(script)?;
                let chunk = vm.lua.load(source).set_name(format!("@{script}"));
                vm.deadline.set(Some(Instant::now() + LOAD_BUDGET));
                let loaded = chunk.eval::<Value>();
                vm.deadline.set(None);
                match loaded {
                    Ok(Value::Table(module)) => {
                        vm.module = Some(module);
                        Outcome::Ok
                    }
                    Ok(_) => Outcome::Ok,
                    Err(error) => vm.strike(error.to_string()),
                }
            }
            None => Outcome::Ok,
        };
        Ok((vm, outcome))
    }

    pub fn host(&self) -> mlua::AppDataRefMut<'_, Host> {
        self.lua
            .app_data_mut::<Host>()
            .expect("the host is installed before any script runs")
    }

    pub fn take_commands(&self) -> Vec<Command> {
        std::mem::take(&mut self.host().commands)
    }

    fn strike(&mut self, message: String) -> Outcome {
        let now = Instant::now();
        self.strikes.push_back(now);
        while self
            .strikes
            .front()
            .is_some_and(|first| now.duration_since(*first) > STRIKE_WINDOW)
        {
            self.strikes.pop_front();
        }
        if self.strikes.len() >= STRIKES {
            Outcome::Quarantine(message)
        } else {
            Outcome::Failed(message)
        }
    }

    pub fn call(&mut self, function: &Function, args: impl IntoLuaMulti) -> Outcome {
        self.deadline.set(Some(Instant::now() + self.budget));
        let result = function.call::<()>(args);
        self.deadline.set(None);
        match result {
            Ok(()) => Outcome::Ok,
            Err(error) => self.strike(error.to_string()),
        }
    }

    fn module_function(&self, path: &[&str]) -> Option<Function> {
        let mut table = self.module.clone()?;
        let (last, parents) = path.split_last()?;
        for parent in parents {
            table = table.get::<Option<Table>>(*parent).ok()??;
        }
        table.get::<Option<Function>>(*last).ok()?
    }

    pub fn action(&mut self, name: &str) -> Outcome {
        match self.module_function(&["actions", name]) {
            Some(function) => self.call(&function, ()),
            None => Outcome::Failed(format!("no handler `actions.{name}`")),
        }
    }

    pub fn popup(&mut self, widget: &str, open: bool) -> Outcome {
        match self.module_function(&["on_popup"]) {
            Some(function) => self.call(&function, (widget.to_owned(), open)),
            None => Outcome::Ok,
        }
    }

    pub fn unload(&mut self) {
        if let Some(function) = self.module_function(&["on_unload"]) {
            let _ = self.call(&function, ());
        }
    }

    pub fn fire_timer(&mut self, id: u32) -> Option<Outcome> {
        let function = self.host().timers.get(&id).cloned()?;
        Some(self.call(&function, ()))
    }

    pub fn complete(&mut self, request: u32, args: impl IntoLuaMulti) -> Outcome {
        let callback = self.host().requests.remove(&request);
        match callback {
            Some(function) => self.call(&function, args),
            None => Outcome::Ok,
        }
    }

    /// Call every listener registered for `event`.
    pub fn emit<A: IntoLuaMulti + Clone>(&mut self, event: &str, args: A) -> Outcome {
        let listeners = self.host().listeners.get(event).cloned().unwrap_or_default();
        let mut outcome = Outcome::Ok;
        for listener in listeners {
            match self.call(&listener, args.clone()) {
                Outcome::Ok => {}
                failed => outcome = failed,
            }
        }
        outcome
    }

    pub fn ui_event(&mut self, widget: &str, part: Part, event: &UiEvent) -> Outcome {
        let handler = self
            .host()
            .handlers
            .get(&(widget.to_owned(), part))
            .and_then(|handlers| handlers.get(event.node().0 as usize).cloned().flatten());
        let Some(handler) = handler else {
            return Outcome::Ok;
        };
        match *event {
            UiEvent::Click(_) => self.call(&handler, ()),
            UiEvent::Toggle(_, on) => self.call(&handler, on),
            UiEvent::Change(_, value) => self.call(&handler, value),
            UiEvent::Select(_, index) => self.call(&handler, index + 1),
        }
    }

    pub fn lua(&self) -> &Lua {
        &self.lua
    }
}

fn lua_error(error: mlua::Error) -> anyhow::Error {
    anyhow::anyhow!("{error}")
}

fn build(
    package: &Package,
    grants: &Grants,
    limits: Limits,
    deadline: Rc<Cell<Option<Instant>>>,
    seed: Seed,
) -> mlua::Result<Lua> {
    let libs = StdLib::TABLE
        | StdLib::STRING
        | StdLib::UTF8
        | StdLib::BIT
        | StdLib::MATH
        | StdLib::BUFFER
        | StdLib::OS;
    let lua = Lua::new_with(libs, LuaOptions::default())?;
    lua.set_interrupt(move |_| match deadline.get() {
        Some(deadline) if Instant::now() > deadline => {
            Err(mlua::Error::runtime("ran past its time budget"))
        }
        _ => Ok(VmState::Continue),
    });
    let mut host = Host::new(package, package.manifest.kind() == Kind::Theme);
    host.dark_mode = seed.dark_mode;
    host.tokens = seed.tokens;
    host.settings = seed.settings;
    lua.set_app_data(host);
    api::install(&lua, package, grants)?;
    lua.sandbox(true)?;
    lua.set_memory_limit(limits.memory_mb as usize * 1024 * 1024)?;
    Ok(lua)
}

#[cfg(test)]
mod tests;
