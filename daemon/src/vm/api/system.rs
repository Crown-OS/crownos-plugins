//! Gated: network, programs, files and notifications.

use crownplugin_model::{Capability, UserDir};
use mlua::{Function, Lua, Table};

use super::{Grants, host, namespace, push};
use crate::vm::{Command, Method};

fn user_dir(name: &str) -> mlua::Result<UserDir> {
    Ok(match name {
        "home" => UserDir::Home,
        "documents" => UserDir::Documents,
        "downloads" => UserDir::Downloads,
        "music" => UserDir::Music,
        "pictures" => UserDir::Pictures,
        "videos" => UserDir::Videos,
        other => return Err(mlua::Error::runtime(format!("unknown folder `{other}`"))),
    })
}

fn allowed(dirs: &[UserDir], name: &str) -> mlua::Result<UserDir> {
    let dir = user_dir(name)?;
    if dirs.contains(&dir) {
        Ok(dir)
    } else {
        Err(mlua::Error::runtime(format!("no access to the {name} folder")))
    }
}

fn install_http(lua: &Lua) -> mlua::Result<()> {
    let http = namespace(lua, "http")?;
    http.set(
        "get",
        lua.create_function(|lua, (url, callback): (String, Option<Function>)| {
            host(lua, |host| {
                let request = host.request(callback);
                host.commands.push(Command::Http { request, method: Method::Get, url, body: None });
            })
        })?,
    )?;
    http.set(
        "post",
        lua.create_function(|lua, (url, body, callback): (String, String, Option<Function>)| {
            host(lua, |host| {
                let request = host.request(callback);
                host.commands.push(Command::Http { request, method: Method::Post, url, body: Some(body) });
            })
        })?,
    )
}

fn install_exec(lua: &Lua) -> mlua::Result<()> {
    let exec = namespace(lua, "exec")?;
    exec.set(
        "run",
        lua.create_function(|lua, (argv, callback): (Vec<String>, Option<Function>)| {
            if argv.is_empty() {
                return Err(mlua::Error::runtime("exec.run needs a program"));
            }
            host(lua, |host| {
                let request = host.request(callback);
                host.commands.push(Command::Exec { request, argv });
            })
        })?,
    )
}

fn install_fs(lua: &Lua, grants: &Grants) -> mlua::Result<()> {
    let fs: Table = namespace(lua, "fs")?;
    let readable = grants.dirs(false);
    let listable = readable.clone();
    fs.set(
        "read",
        lua.create_function(move |lua, (dir, path, callback): (String, String, Option<Function>)| {
            let dir = allowed(&readable, &dir)?;
            host(lua, |host| {
                let request = host.request(callback);
                host.commands.push(Command::ReadFile { request, dir, path });
            })
        })?,
    )?;
    fs.set(
        "list",
        lua.create_function(move |lua, (dir, path, callback): (String, String, Option<Function>)| {
            let dir = allowed(&listable, &dir)?;
            host(lua, |host| {
                let request = host.request(callback);
                host.commands.push(Command::ListDir { request, dir, path });
            })
        })?,
    )?;
    let writable = grants.dirs(true);
    if !writable.is_empty() {
        fs.set(
            "write",
            lua.create_function(
                move |lua, (dir, path, text, callback): (String, String, String, Option<Function>)| {
                    let dir = allowed(&writable, &dir)?;
                    host(lua, |host| {
                        let request = host.request(callback);
                        host.commands.push(Command::WriteFile { request, dir, path, text });
                    })
                },
            )?,
        )?;
    }
    Ok(())
}

fn install_notify(lua: &Lua) -> mlua::Result<()> {
    let notify = namespace(lua, "notify")?;
    notify.set(
        "send",
        lua.create_function(|lua, options: Table| {
            let summary: String = options.get("summary")?;
            let body: Option<String> = options.get("body")?;
            push(lua, Command::Notify { summary, body: body.unwrap_or_default() })
        })?,
    )
}

pub fn install(lua: &Lua, grants: &Grants) -> mlua::Result<()> {
    if grants.find(|capability| matches!(capability, Capability::Network { .. })).is_some() {
        install_http(lua)?;
    }
    if grants.find(|capability| matches!(capability, Capability::Exec { .. })).is_some() {
        install_exec(lua)?;
    }
    if !grants.dirs(false).is_empty() || !grants.dirs(true).is_empty() {
        install_fs(lua, grants)?;
    }
    if grants.has(&Capability::Notify) {
        install_notify(lua)?;
    }
    Ok(())
}
