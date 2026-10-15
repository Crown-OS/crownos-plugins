use serde::{Deserialize, Serialize};

/// Identifies an interactive node within one tree; assigned by the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u32);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UiEvent {
    Click(NodeId),
    Toggle(NodeId, bool),
    Change(NodeId, f32),
    Select(NodeId, u32),
}

impl UiEvent {
    pub fn node(&self) -> NodeId {
        match self {
            Self::Click(id) | Self::Toggle(id, _) | Self::Change(id, _) | Self::Select(id, _) => *id,
        }
    }
}
