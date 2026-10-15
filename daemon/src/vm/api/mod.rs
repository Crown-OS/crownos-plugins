//! The globals a script sees. A group of functions exists only when the
//! entity declared the capability behind it and the user has not denied it.

mod core;
mod desktop;
mod system;
mod ui;

use std::collections::HashSet;

use crownplugin_model::{Capability, Package, UserDir};
use mlua::{Lua, Table};

use super::{Command, Host};

/// What the entity may use when its VM is built.
#[derive(Debug, Clone, Default)]
pub struct Grants {
    pub declared: Vec<Capability>,
    /// Grant keys the user said no to.
    pub denied: HashSet<String>,
}

impl Grants {
    pub fn has(&self, capability: &Capability) -> bool {
        self.declared.contains(capability) && !self.denied.contains(capability.grant_key().as_ref())
    }

    pub fn find(&self, matches: impl Fn(&Capability) -> bool) -> Option<&Capability> {
        self.declared
            .iter()
            .find(|capability| matches(capability) && !self.denied.contains(capability.grant_key().as_ref()))
    }

    pub fn dirs(&self, write: bool) -> Vec<UserDir> {
        self.declared
            .iter()
            .filter(|capability| !self.denied.contains(capability.grant_key().as_ref()))
            .filter_map(|capability| match (capability, write) {
                (Capability::FsRead(dir), false) | (Capability::FsWrite(dir), true) => Some(*dir),
                _ => None,
            })
            .collect()
    }
}

pub(super) fn push(lua: &Lua, command: Command) -> mlua::Result<()> {
    host(lua, |host| host.commands.push(command))
}

pub(super) fn host<R>(lua: &Lua, f: impl FnOnce(&mut Host) -> R) -> mlua::Result<R> {
    let mut host = lua
        .app_data_mut::<Host>()
        .ok_or_else(|| mlua::Error::runtime("host missing"))?;
    Ok(f(&mut host))
}

pub(super) fn namespace(lua: &Lua, name: &str) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    lua.globals().set(name, &table)?;
    Ok(table)
}

pub fn install(lua: &Lua, package: &Package, grants: &Grants) -> mlua::Result<()> {
    core::install(lua, package)?;
    ui::install(lua)?;
    system::install(lua, grants)?;
    desktop::install(lua, grants)?;
    Ok(())
}
