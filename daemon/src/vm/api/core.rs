//! Always available: logging, timers, modules, settings, theme and feeds.

use std::time::Duration;

use crownplugin_model::{Package, SettingValue, package::relative_path};
use crownplugin_proto::LogLevel;
use mlua::{Function, Lua, Table, Value};

use super::{host, namespace, push};
use crate::feeds;
use crate::vm::Command;

const MIN_REPEAT: Duration = Duration::from_millis(250);
const MIN_DELAY: Duration = Duration::from_millis(16);

pub fn to_lua(lua: &Lua, value: &SettingValue) -> mlua::Result<Value> {
    Ok(match value {
        SettingValue::Bool(value) => Value::Boolean(*value),
        SettingValue::Number(value) => Value::Number(*value),
        SettingValue::Text(value) => Value::String(lua.create_string(value)?),
    })
}

pub fn from_lua(value: Value) -> mlua::Result<SettingValue> {
    Ok(match value {
        Value::Boolean(value) => SettingValue::Bool(value),
        Value::Integer(value) => SettingValue::Number(value as f64),
        Value::Number(value) => SettingValue::Number(value),
        Value::String(value) => SettingValue::Text(value.to_str()?.to_owned()),
        other => {
            return Err(mlua::Error::runtime(format!(
                "a setting is a boolean, number or string, not {}",
                other.type_name()
            )));
        }
    })
}

fn listen(lua: &Lua, event: &'static str) -> mlua::Result<Function> {
    lua.create_function(move |lua, callback: Function| {
        host(lua, |host| host.listeners.entry(event.to_owned()).or_default().push(callback))
    })
}

fn install_log(lua: &Lua) -> mlua::Result<()> {
    let log = namespace(lua, "log")?;
    for (name, level) in [
        ("debug", LogLevel::Debug),
        ("info", LogLevel::Info),
        ("warn", LogLevel::Warn),
        ("error", LogLevel::Error),
    ] {
        log.set(
            name,
            lua.create_function(move |lua, message: String| push(lua, Command::Log(level, message)))?,
        )?;
    }
    lua.globals().set(
        "print",
        lua.create_function(|lua, values: mlua::Variadic<Value>| {
            let line: Vec<String> = values
                .iter()
                .map(|value| value.to_string().unwrap_or_else(|_| value.type_name().to_owned()))
                .collect();
            push(lua, Command::Log(LogLevel::Info, line.join("\t")))
        })?,
    )
}

fn install_timer(lua: &Lua) -> mlua::Result<()> {
    let timer = namespace(lua, "timer")?;
    for (name, repeat) in [("every", true), ("after", false)] {
        timer.set(
            name,
            lua.create_function(move |lua, (ms, callback): (f64, Function)| {
                let floor = if repeat { MIN_REPEAT } else { MIN_DELAY };
                let every = Duration::from_secs_f64(ms.max(0.0) / 1000.0).max(floor);
                host(lua, |host| {
                    let id = host.next_id();
                    host.timers.insert(id, callback);
                    host.commands.push(Command::Timer { id, every, repeat });
                    id
                })
            })?,
        )?;
    }
    timer.set(
        "cancel",
        lua.create_function(|lua, id: u32| {
            host(lua, |host| {
                host.timers.remove(&id);
                host.commands.push(Command::CancelTimer(id));
            })
        })?,
    )
}

fn install_require(lua: &Lua, package: &Package) -> mlua::Result<()> {
    let dir = package.dir.clone();
    lua.globals().set(
        "require",
        lua.create_function(move |lua, name: String| {
            if let Some(cached) = host(lua, |host| host.modules.get(&name).cloned())? {
                return Ok(cached);
            }
            let file = if name.ends_with(".luau") { name.clone() } else { format!("{name}.luau") };
            let path = relative_path(&file)
                .map(|rel| dir.join(rel))
                .ok_or_else(|| mlua::Error::runtime(format!("`{name}` is outside the package")))?;
            let source = std::fs::read_to_string(&path)
                .map_err(|error| mlua::Error::runtime(format!("require `{name}`: {error}")))?;
            let value: Value = lua.load(source).set_name(format!("@{file}")).eval()?;
            host(lua, |host| host.modules.insert(name, value.clone()))?;
            Ok(value)
        })?,
    )
}

fn install_settings(lua: &Lua) -> mlua::Result<()> {
    let settings = namespace(lua, "settings")?;
    settings.set(
        "get",
        lua.create_function(|lua, key: String| {
            let value = host(lua, |host| host.settings.get(&key).cloned())?;
            value.map(|value| to_lua(lua, &value)).transpose()
        })?,
    )?;
    settings.set(
        "all",
        lua.create_function(|lua, ()| {
            let values = host(lua, |host| host.settings.clone())?;
            let table = lua.create_table()?;
            for (key, value) in &values {
                table.set(key.as_str(), to_lua(lua, value)?)?;
            }
            Ok(table)
        })?,
    )?;
    settings.set(
        "set",
        lua.create_function(|lua, (key, value): (String, Value)| {
            let value = from_lua(value)?;
            host(lua, |host| {
                host.settings.insert(key.clone(), value.clone());
                host.commands.push(Command::SetSetting { key, value });
            })
        })?,
    )?;
    settings.set("on_change", listen(lua, "settings")?)
}

fn install_theme(lua: &Lua) -> mlua::Result<()> {
    let theme = namespace(lua, "theme")?;
    theme.set("dark", lua.create_function(|lua, ()| host(lua, |host| host.dark_mode))?)?;
    theme.set(
        "token",
        lua.create_function(|lua, name: String| host(lua, |host| host.tokens.get(&name).cloned()))?,
    )?;
    theme.set("on_change", listen(lua, "theme")?)
}

fn install_feed(lua: &Lua) -> mlua::Result<()> {
    let feed = namespace(lua, "feed")?;
    feed.set(
        "time",
        lua.create_function(|lua, ()| {
            let now = feeds::time::now();
            let table = lua.create_table()?;
            table.set("hour", now.hour)?;
            table.set("minute", now.minute)?;
            table.set("second", now.second)?;
            table.set("year", now.year)?;
            table.set("month", now.month)?;
            table.set("day", now.day)?;
            table.set("weekday", now.weekday)?;
            table.set("unix", now.unix)?;
            Ok(table)
        })?,
    )?;
    feed.set("format", lua.create_function(|_, pattern: String| Ok(feeds::time::format(&pattern)))?)?;
    feed.set(
        "system",
        lua.create_function(|lua, ()| {
            let info = host(lua, |host| host.sampler.sample())?;
            let table = lua.create_table()?;
            table.set("cpu", info.cpu)?;
            table.set("memory", info.memory)?;
            table.set("memory_used_mb", info.memory_used_mb)?;
            table.set("memory_total_mb", info.memory_total_mb)?;
            table.set("uptime", info.uptime_secs)?;
            Ok(table)
        })?,
    )?;
    feed.set(
        "power",
        lua.create_function(|lua, ()| {
            let info = feeds::power::read();
            let table = lua.create_table()?;
            table.set("present", info.present)?;
            table.set("percent", info.percent)?;
            table.set("charging", info.charging)?;
            Ok(table)
        })?,
    )?;
    Ok(())
}

pub fn install(lua: &Lua, package: &Package) -> mlua::Result<()> {
    let plugin: Table = namespace(lua, "plugin")?;
    plugin.set("id", package.id().as_str())?;
    plugin.set("dir", package.dir.to_string_lossy().as_ref())?;
    install_log(lua)?;
    install_timer(lua)?;
    install_require(lua, package)?;
    install_settings(lua)?;
    install_theme(lua)?;
    install_feed(lua)
}
