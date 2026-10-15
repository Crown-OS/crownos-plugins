//! The latest tree of every plugin surface, versioned per part.

use std::collections::BTreeMap;

use crownplugin_model::LockSlotPosition;
use crownplugin_proto::{Part, RemoteSurface, Snapshot};
use crownplugin_view::{MAX_TREE_BYTES, Node, Tree};

#[derive(Debug, Default)]
struct Surface {
    main: Option<Tree>,
    popup: Option<Tree>,
    lock_slot: Option<LockSlotPosition>,
}

#[derive(Debug, Default)]
pub struct Surfaces {
    bar: BTreeMap<(String, String), Surface>,
    lock: BTreeMap<(String, String), Surface>,
    lock_layout: Option<Tree>,
    revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Changed {
    Nothing,
    Bar,
    Lock,
}

fn put(slot: &mut Option<Tree>, node: Option<Node>) -> bool {
    match (slot.as_mut(), node) {
        (Some(tree), Some(node)) if tree.root == node => false,
        (Some(tree), Some(node)) => {
            tree.rev = tree.rev.wrapping_add(1);
            tree.root = node;
            true
        }
        (None, Some(node)) => {
            *slot = Some(Tree { rev: 1, root: node });
            true
        }
        (Some(_), None) => {
            *slot = None;
            true
        }
        (None, None) => false,
    }
}

impl Surfaces {
    /// Store a rendered tree; `lock` is the slot position for lock surfaces.
    pub fn render(
        &mut self,
        plugin: &str,
        widget: &str,
        part: Part,
        node: Option<Node>,
        lock: Option<LockSlotPosition>,
    ) -> Result<Changed, String> {
        if let Some(node) = &node {
            let size = postcard_len(node);
            if size > MAX_TREE_BYTES {
                return Err(format!("`{widget}` renders {size} bytes; the limit is {MAX_TREE_BYTES}"));
            }
        }
        let key = (plugin.to_owned(), widget.to_owned());
        let (map, changed) = match lock {
            Some(_) => (&mut self.lock, Changed::Lock),
            None => (&mut self.bar, Changed::Bar),
        };
        let surface = map.entry(key).or_default();
        surface.lock_slot = lock;
        let slot = match part {
            Part::Main => &mut surface.main,
            Part::Popup => &mut surface.popup,
        };
        if !put(slot, node) {
            return Ok(Changed::Nothing);
        }
        self.revision += 1;
        Ok(changed)
    }

    pub fn set_lock_layout(&mut self, node: Option<Node>) -> bool {
        let changed = put(&mut self.lock_layout, node);
        if changed {
            self.revision += 1;
        }
        changed
    }

    pub fn lock_layout(&self) -> Option<&Tree> {
        self.lock_layout.as_ref()
    }

    /// Forget everything `plugin` drew; reports which hosts must hear about it.
    pub fn remove(&mut self, plugin: &str) -> (bool, bool) {
        let before = (self.bar.len(), self.lock.len());
        self.bar.retain(|(owner, _), _| owner != plugin);
        self.lock.retain(|(owner, _), _| owner != plugin);
        let changed = (self.bar.len() != before.0, self.lock.len() != before.1);
        if changed.0 || changed.1 {
            self.revision += 1;
        }
        changed
    }

    pub fn current_rev(&self, plugin: &str, widget: &str, part: Part) -> Option<u32> {
        let key = (plugin.to_owned(), widget.to_owned());
        let surface = self.bar.get(&key).or_else(|| self.lock.get(&key))?;
        match part {
            Part::Main => surface.main.as_ref(),
            Part::Popup => surface.popup.as_ref(),
        }
        .map(|tree| tree.rev)
    }

    fn snapshot(&self, map: &BTreeMap<(String, String), Surface>) -> Snapshot {
        Snapshot {
            revision: self.revision,
            surfaces: map
                .iter()
                .map(|((plugin, widget), surface)| RemoteSurface {
                    plugin: plugin.clone(),
                    widget: widget.clone(),
                    main: surface.main.clone(),
                    popup: surface.popup.clone(),
                    lock_slot: surface.lock_slot,
                })
                .collect(),
        }
    }

    pub fn bar(&self) -> Snapshot {
        self.snapshot(&self.bar)
    }

    pub fn lock(&self) -> Snapshot {
        self.snapshot(&self.lock)
    }
}

fn postcard_len(node: &Node) -> usize {
    crownos_ipc::postcard::to_stdvec(node).map_or(usize::MAX, |bytes| bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Node {
        Node::Text { style: Vec::new(), text: value.into() }
    }

    #[test]
    fn identical_renders_keep_their_revision() {
        let mut surfaces = Surfaces::default();
        assert_eq!(surfaces.render("p", "w", Part::Main, Some(text("a")), None), Ok(Changed::Bar));
        assert_eq!(surfaces.render("p", "w", Part::Main, Some(text("a")), None), Ok(Changed::Nothing));
        assert_eq!(surfaces.current_rev("p", "w", Part::Main), Some(1));
        surfaces.render("p", "w", Part::Main, Some(text("b")), None).expect("fits");
        assert_eq!(surfaces.current_rev("p", "w", Part::Main), Some(2));
    }

    #[test]
    fn removing_a_plugin_clears_its_surfaces() {
        let mut surfaces = Surfaces::default();
        surfaces.render("p", "w", Part::Main, Some(text("a")), None).expect("fits");
        surfaces
            .render("p", "slot", Part::Main, Some(text("b")), Some(LockSlotPosition::Bottom))
            .expect("fits");
        assert_eq!(surfaces.remove("p"), (true, true));
        assert!(surfaces.bar().surfaces.is_empty() && surfaces.lock().surfaces.is_empty());
    }
}
