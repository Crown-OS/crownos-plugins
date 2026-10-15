use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ModelError;

/// Reverse-DNS identifier such as `dev.alice.cpu-graph`; also the install
/// directory name, so it is restricted to path-safe characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EntityId(String);

const MIN_LEN: usize = 3;
const MAX_LEN: usize = 64;

impl EntityId {
    pub fn new(id: impl Into<String>) -> Result<Self, ModelError> {
        let id = id.into();
        let reason = if !(MIN_LEN..=MAX_LEN).contains(&id.len()) {
            Some("must be 3 to 64 characters")
        } else if !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".-_".contains(&byte))
        {
            Some("only a-z, 0-9, '.', '-' and '_' are allowed")
        } else if !id.contains('.') || id.starts_with('.') || id.ends_with('.') || id.contains("..")
        {
            Some("must be reverse-DNS, like `dev.alice.cpu-graph`")
        } else {
            None
        };
        match reason {
            Some(reason) => Err(ModelError::InvalidId { id, reason }),
            None => Ok(Self(id)),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The crownos-config section holding this entity's own settings.
    pub fn settings_section(&self) -> String {
        format!("plugin.{}", self.0)
    }
}

impl TryFrom<String> for EntityId {
    type Error = ModelError;

    fn try_from(id: String) -> Result<Self, Self::Error> {
        Self::new(id)
    }
}

impl From<EntityId> for String {
    fn from(id: EntityId) -> Self {
        id.0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for EntityId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_dns_ids_are_accepted() {
        assert!(EntityId::new("dev.alice.cpu-graph").is_ok());
        assert!(EntityId::new("crown.midnight_2").is_ok());
    }

    #[test]
    fn unsafe_or_flat_ids_are_refused() {
        for id in ["cpu", "Dev.Alice", "dev/alice", "../etc", ".dev.x", "dev..x", "a.b c"] {
            assert!(EntityId::new(id).is_err(), "{id} should be refused");
        }
    }
}
