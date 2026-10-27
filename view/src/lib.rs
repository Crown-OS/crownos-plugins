//! UI as data. A plugin builds a [`Tree`] of crownui-named nodes styled with
//! Tailwind-style classes; every host renders it with its own toolkit, so a
//! plugin can never paint outside the system's look.

pub mod class;
mod event;
#[cfg(feature = "icons")]
mod icons;
#[cfg(feature = "icons")]
mod lucide;
mod node;
mod style;

pub use class::{ClassError, parse_class};
pub use event::{NodeId, UiEvent};
#[cfg(feature = "icons")]
pub use icons::{lucide, to_svg};
pub use node::{ButtonVariant, Node, Tone, Tree};
pub use style::{Align, Justify, Length, Radius, Style, TextSize, Token, Weight};

/// Largest encoded tree a host accepts.
pub const MAX_TREE_BYTES: usize = 256 * 1024;
