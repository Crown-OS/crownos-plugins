//! `launcher`: answering search queries with hits.

use crownplugin_proto::SearchHit;
use mlua::{Lua, Table};

use super::{host, namespace, push};
use crate::vm::Command;

/// More than a launcher can show for one provider.
const MAX_HITS: usize = 20;
const MAX_TEXT: usize = 200;

fn field(table: &Table, key: &str) -> mlua::Result<Option<String>> {
    Ok(table
        .get::<Option<String>>(key)?
        .map(|text| text.chars().take(MAX_TEXT).collect()))
}

fn hit(table: &Table) -> mlua::Result<SearchHit> {
    let title = field(table, "title")?.ok_or_else(|| mlua::Error::runtime("a hit needs a `title`"))?;
    Ok(SearchHit {
        id: field(table, "id")?.unwrap_or_else(|| title.clone()),
        subtitle: field(table, "subtitle")?.unwrap_or_default(),
        icon: field(table, "icon")?,
        title,
    })
}

fn results(lua: &Lua, (provider, query, hits): (String, String, Option<Table>)) -> mlua::Result<()> {
    if !host(lua, |host| host.search_providers.contains(&provider))? {
        return Err(mlua::Error::runtime(format!("`{provider}` is not declared in contributes.search")));
    }
    let hits = match hits {
        Some(hits) => hits
            .sequence_values::<Table>()
            .take(MAX_HITS)
            .map(|table| hit(&table?))
            .collect::<mlua::Result<Vec<_>>>()?,
        None => Vec::new(),
    };
    push(lua, Command::Answer { provider, query, hits })
}

pub fn install(lua: &Lua) -> mlua::Result<()> {
    let launcher = namespace(lua, "launcher")?;
    launcher.set("results", lua.create_function(results)?)
}
