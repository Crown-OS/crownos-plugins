use serde::{Deserialize, Serialize};

use crate::{NodeId, Style};

/// One rendered surface, versioned so a host can drop stale updates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tree {
    pub rev: u32,
    pub root: Node,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Tone {
    #[default]
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Ghost,
    Danger,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Node {
    VStack { style: Vec<Style>, children: Vec<Node> },
    HStack { style: Vec<Style>, children: Vec<Node> },
    ZStack { style: Vec<Style>, children: Vec<Node> },
    Card { style: Vec<Style>, children: Vec<Node> },
    Text { style: Vec<Style>, text: String },
    /// A Lucide icon by name, e.g. `cpu`.
    Icon { style: Vec<Style>, name: String },
    /// A package-relative image path.
    Image { style: Vec<Style>, src: String },
    Spacer,
    Divider,
    Badge { text: String, tone: Tone },
    Progress { style: Vec<Style>, value: f32 },
    Button {
        id: NodeId,
        style: Vec<Style>,
        label: String,
        icon: Option<String>,
        variant: ButtonVariant,
    },
    Toggle { id: NodeId, label: String, on: bool },
    Slider { id: NodeId, value: f32, min: f32, max: f32, step: f32 },
    Select { id: NodeId, options: Vec<String>, selected: u32 },
    /// A system part the host draws itself, e.g. `lock.clock` or
    /// `lock.secure_field`. Only valid in lock-screen trees.
    Placeholder { name: String },
}

impl Node {
    pub fn children(&self) -> &[Node] {
        match self {
            Self::VStack { children, .. }
            | Self::HStack { children, .. }
            | Self::ZStack { children, .. }
            | Self::Card { children, .. } => children,
            _ => &[],
        }
    }

    /// Whether the node takes input. Lock-screen slots must contain none.
    pub fn is_interactive(&self) -> bool {
        matches!(
            self,
            Self::Button { .. } | Self::Toggle { .. } | Self::Slider { .. } | Self::Select { .. }
        )
    }

    pub fn any(&self, predicate: &impl Fn(&Node) -> bool) -> bool {
        predicate(self) || self.children().iter().any(|child| child.any(predicate))
    }

    /// Strip every interactive node, for display-only surfaces.
    pub fn into_display_only(self) -> Self {
        let keep = |children: Vec<Node>| {
            children
                .into_iter()
                .filter(|child| !child.is_interactive())
                .map(Node::into_display_only)
                .collect()
        };
        match self {
            Self::VStack { style, children } => Self::VStack { style, children: keep(children) },
            Self::HStack { style, children } => Self::HStack { style, children: keep(children) },
            Self::ZStack { style, children } => Self::ZStack { style, children: keep(children) },
            Self::Card { style, children } => Self::Card { style, children: keep(children) },
            node if node.is_interactive() => Self::Spacer,
            node => node,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_only_drops_every_input() {
        let tree = Node::VStack {
            style: vec![],
            children: vec![
                Node::Text { style: vec![], text: "12:00".into() },
                Node::HStack {
                    style: vec![],
                    children: vec![Node::Toggle { id: NodeId(1), label: "x".into(), on: false }],
                },
            ],
        };
        assert!(tree.any(&Node::is_interactive));
        assert!(!tree.into_display_only().any(&Node::is_interactive));
    }
}
