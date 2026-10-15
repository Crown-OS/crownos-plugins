//! `ui`: node builders from the prelude, plus `ui.render` and `ui.lock_layout`.

use crownplugin_proto::Part;
use mlua::{Lua, Table, Value};

use super::{host, push};
use crate::vm::Command;
use crate::vm::convert::Converter;

const PRELUDE: &str = include_str!("../prelude.luau");

enum Target {
    Bar,
    Lock,
}

fn render(lua: &Lua, (widget, main, popup): (String, Value, Value)) -> mlua::Result<()> {
    let target = host(lua, |host| {
        if host.bar_widgets.contains(&widget) {
            Some(Target::Bar)
        } else if host.lock_slots.iter().any(|(id, _)| *id == widget) {
            Some(Target::Lock)
        } else {
            None
        }
    })?
    .ok_or_else(|| {
        mlua::Error::runtime(format!(
            "`{widget}` is not declared in contributes.bar or contributes.lock"
        ))
    })?;
    let parts = match target {
        Target::Bar => vec![(Part::Main, main), (Part::Popup, popup)],
        Target::Lock => vec![(Part::Main, main)],
    };
    for (part, value) in parts {
        let mut converter = Converter::new(false);
        let tree = match value {
            Value::Nil => None,
            value => Some(converter.convert(value)?),
        };
        let tree = match target {
            Target::Lock => tree.map(crownplugin_view::Node::into_display_only),
            Target::Bar => tree,
        };
        host(lua, |host| {
            host.handlers.insert((widget.clone(), part), converter.handlers);
            host.commands.push(Command::Render {
                widget: widget.clone(),
                part,
                tree,
            });
        })?;
    }
    Ok(())
}

fn lock_layout(lua: &Lua, value: Value) -> mlua::Result<()> {
    if !host(lua, |host| host.is_theme)? {
        return Err(mlua::Error::runtime("only a theme can lay out the lock screen"));
    }
    let tree = match value {
        Value::Nil => None,
        value => Some(Converter::new(true).convert(value)?.into_display_only()),
    };
    push(lua, Command::LockDecoration(tree))
}

pub fn install(lua: &Lua) -> mlua::Result<()> {
    let ui: Table = lua.load(PRELUDE).set_name("@ui").eval()?;
    ui.set("render", lua.create_function(render)?)?;
    ui.set("lock_layout", lua.create_function(lock_layout)?)?;
    lua.globals().set("ui", ui)
}
