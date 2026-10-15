//! Luau node tables (built by the `ui` prelude) to [`Node`]s.

use crownplugin_view::{ButtonVariant, Node, NodeId, Tone, parse_class};
use mlua::{Function, Table, Value};

const MAX_NODES: usize = 2000;
const MAX_DEPTH: usize = 32;

/// Converts one tree, collecting the handlers of its interactive nodes in
/// [`NodeId`] order.
pub struct Converter {
    pub handlers: Vec<Option<Function>>,
    nodes: usize,
    placeholders: bool,
}

fn error(message: impl Into<String>) -> mlua::Error {
    mlua::Error::runtime(message.into())
}

impl Converter {
    pub fn new(placeholders: bool) -> Self {
        Self {
            handlers: Vec::new(),
            nodes: 0,
            placeholders,
        }
    }

    pub fn convert(&mut self, value: Value) -> mlua::Result<Node> {
        self.node(value, 0)
    }

    fn handler(&mut self, table: &Table, field: &str) -> mlua::Result<NodeId> {
        let id = NodeId(self.handlers.len() as u32);
        self.handlers.push(table.get::<Option<Function>>(field)?);
        Ok(id)
    }

    fn node(&mut self, value: Value, depth: usize) -> mlua::Result<Node> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(error(format!("a tree may hold at most {MAX_NODES} nodes")));
        }
        if depth > MAX_DEPTH {
            return Err(error(format!("a tree may nest at most {MAX_DEPTH} levels")));
        }
        let table = match value {
            Value::Table(table) => table,
            Value::String(text) => {
                return Ok(Node::Text {
                    style: Vec::new(),
                    text: text.to_str()?.to_owned(),
                });
            }
            other => {
                return Err(error(format!(
                    "expected a ui node, got {}; build nodes with ui.text, ui.hstack, ...",
                    other.type_name()
                )));
            }
        };
        let kind: String = table.get("kind")?;
        let class: Option<String> = table.get("class")?;
        let style = class
            .as_deref()
            .map(parse_class)
            .transpose()
            .map_err(|error| mlua::Error::runtime(error.to_string()))?
            .unwrap_or_default();
        let string = |field: &str| -> mlua::Result<String> {
            Ok(table.get::<Option<String>>(field)?.unwrap_or_default())
        };
        Ok(match kind.as_str() {
            "vstack" | "hstack" | "zstack" | "card" => {
                let children = table
                    .get::<Option<Table>>("children")?
                    .map(|children| {
                        children
                            .sequence_values::<Value>()
                            .map(|child| self.node(child?, depth + 1))
                            .collect::<mlua::Result<Vec<_>>>()
                    })
                    .transpose()?
                    .unwrap_or_default();
                match kind.as_str() {
                    "vstack" => Node::VStack { style, children },
                    "hstack" => Node::HStack { style, children },
                    "zstack" => Node::ZStack { style, children },
                    _ => Node::Card { style, children },
                }
            }
            "text" => Node::Text { style, text: string("text")? },
            "icon" => Node::Icon { style, name: string("name")? },
            "image" => Node::Image { style, src: string("src")? },
            "spacer" => Node::Spacer,
            "divider" => Node::Divider,
            "badge" => Node::Badge {
                text: string("text")?,
                tone: match string("tone")?.as_str() {
                    "accent" => Tone::Accent,
                    "success" => Tone::Success,
                    "warning" => Tone::Warning,
                    "danger" => Tone::Danger,
                    _ => Tone::Neutral,
                },
            },
            "progress" => Node::Progress {
                style,
                value: table.get::<Option<f32>>("value")?.unwrap_or(0.0).clamp(0.0, 1.0),
            },
            "button" => Node::Button {
                id: self.handler(&table, "on_click")?,
                style,
                label: string("label")?,
                icon: table.get("icon")?,
                variant: match string("variant")?.as_str() {
                    "secondary" => ButtonVariant::Secondary,
                    "ghost" => ButtonVariant::Ghost,
                    "danger" => ButtonVariant::Danger,
                    _ => ButtonVariant::Primary,
                },
            },
            "toggle" => Node::Toggle {
                id: self.handler(&table, "on_change")?,
                label: string("label")?,
                on: table.get::<Option<bool>>("on")?.unwrap_or(false),
            },
            "slider" => {
                let min = table.get::<Option<f32>>("min")?.unwrap_or(0.0);
                let max = table.get::<Option<f32>>("max")?.unwrap_or(1.0).max(min);
                Node::Slider {
                    id: self.handler(&table, "on_change")?,
                    value: table.get::<Option<f32>>("value")?.unwrap_or(min).clamp(min, max),
                    min,
                    max,
                    step: table.get::<Option<f32>>("step")?.unwrap_or(0.01).max(f32::EPSILON),
                }
            }
            "select" => {
                let options: Vec<String> = table.get::<Option<Vec<String>>>("options")?.unwrap_or_default();
                let selected = table.get::<Option<u32>>("selected")?.unwrap_or(1).max(1) - 1;
                Node::Select {
                    id: self.handler(&table, "on_change")?,
                    selected: selected.min(options.len().saturating_sub(1) as u32),
                    options,
                }
            }
            "placeholder" if self.placeholders => Node::Placeholder { name: string("name")? },
            "placeholder" => return Err(error("ui.placeholder is only valid in a lock-screen layout")),
            other => return Err(error(format!("unknown ui node kind `{other}`"))),
        })
    }
}
