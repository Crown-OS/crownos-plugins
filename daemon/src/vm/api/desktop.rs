//! The compositor and the user's config.

use crownplugin_model::package::relative_path;
use crownplugin_model::{Capability, ConfigAccess, config_access};
use mlua::{Function, Lua};

use super::{Grants, host, namespace, push};
use crate::vm::Command;

const WINDOW_EVENTS: [&str; 4] = ["window_opened", "window_changed", "window_closed", "window_focused"];

fn install_compositor(lua: &Lua, grants: &Grants) -> mlua::Result<()> {
    let compositor = namespace(lua, "compositor")?;
    if grants.has(&Capability::CompositorActions) {
        compositor.set(
            "run",
            lua.create_function(|lua, action: String| push(lua, Command::RunAction(action)))?,
        )?;
    }
    let windows = grants.has(&Capability::WindowFeed);
    compositor.set(
        "on",
        lua.create_function(move |lua, (event, callback): (String, Function)| {
            let is_window = WINDOW_EVENTS.contains(&event.as_str());
            if is_window && !windows {
                return Err(mlua::Error::runtime("window events need the WindowFeed capability"));
            }
            if !is_window && event != "workspace_changed" && event != "lock_state" {
                return Err(mlua::Error::runtime(format!("unknown compositor event `{event}`")));
            }
            host(lua, |host| {
                host.listeners.entry(event.clone()).or_default().push(callback);
                host.commands.push(Command::Watch(event));
            })
        })?,
    )
}

fn install_config(lua: &Lua, grants: &Grants) -> mlua::Result<()> {
    let visual = grants.has(&Capability::ThemeVisual);
    let declared = grants.clone();
    let config = namespace(lua, "config")?;
    config.set(
        "set",
        lua.create_function(move |lua, (section, key, value): (String, String, String)| {
            let permitted = match config_access(&section, &key) {
                ConfigAccess::Visual => visual,
                ConfigAccess::Needs(capability) => declared.has(&capability),
                ConfigAccess::Forbidden => false,
            };
            if !permitted {
                return Err(mlua::Error::runtime(format!(
                    "not allowed to change {section}.{key}; declare the capability it needs"
                )));
            }
            crownos_config::schema::check_value(&section, &key, &value)
                .map_err(|error| mlua::Error::runtime(error.to_string()))?;
            push(lua, Command::SetConfig { section, key, value })
        })?,
    )?;
    if visual {
        let wallpaper = namespace(lua, "wallpaper")?;
        wallpaper.set(
            "set",
            lua.create_function(|lua, rel: String| {
                let path = host(lua, |host| relative_path(&rel).map(|rel| host.dir.join(rel)))?
                    .filter(|path| path.is_file())
                    .ok_or_else(|| mlua::Error::runtime(format!("`{rel}` is not an image in the package")))?;
                push(lua, Command::SetWallpaper(path))
            })?,
        )?;
    }
    Ok(())
}

pub fn install(lua: &Lua, grants: &Grants) -> mlua::Result<()> {
    install_compositor(lua, grants)?;
    install_config(lua, grants)
}
