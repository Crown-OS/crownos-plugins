//! Plugin trees as crownui views.
//!
//! Every color comes from the host's theme by token, so a plugin cannot
//! paint outside the system palette. The crate depends on crownui's view and
//! widget layers only, so the compositor can run it on its lock-UI thread.

mod nodes;
mod style;

use std::rc::Rc;

use crownos_config::schema::theme::Radii;
use crownplugin_view::{Tree, UiEvent};
use geometry::Color;
use graphics::ImageId;
use reactive::Runtime;
use view::{AnyView, Cx};

pub use crownplugin_view::lucide;

/// Delivers a user's action on an interactive node back to the host.
pub type EventSink = Rc<dyn Fn(UiEvent)>;
/// Resolves tokens the kit palette does not carry, such as `bar.fg`.
pub type TokenSource = Rc<dyn Fn(&mut Runtime, &str) -> Option<Color>>;

/// What the embedding host supplies. Every method has a neutral default.
pub trait Resolver {
    /// A system part the host draws itself, named by a `Placeholder` node.
    fn placeholder(&self, _name: &str, _cx: &mut Cx) -> Option<AnyView> {
        None
    }

    /// An image the host has registered, by the node's `src`.
    fn image(&self, _src: &str) -> Option<ImageId> {
        None
    }

    fn events(&self) -> Option<EventSink> {
        None
    }

    fn tokens(&self) -> Option<TokenSource> {
        None
    }

    fn radii(&self) -> Radii {
        Radii::default()
    }

    fn mono_family(&self) -> String {
        "JetBrains Mono".to_owned()
    }
}

/// The view for `tree`, themed by the theme provided to `cx`.
pub fn render(cx: &mut Cx, tree: &Tree, resolver: &dyn Resolver) -> AnyView {
    nodes::Builder::new(cx, resolver).node(cx, &tree.root)
}
