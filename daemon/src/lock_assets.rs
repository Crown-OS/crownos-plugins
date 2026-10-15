//! Images the lock screen draws, named by SHA-256 and handed to the
//! compositor before anything references them.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crownos_config::schema::{Lock, LockBackground, LockSlotPosition, Theme};
use crownplugin_view::{Node, Tree};
use lockscene::{ASSET_SRC_PREFIX, AssetId, Limits, LockScene};
use sha2::{Digest, Sha256};

use crate::registry::Registry;
use crate::surfaces::Surfaces;

#[derive(Default)]
pub struct LockAssets {
    /// path -> (modified, length, id), so an unchanged file is hashed once.
    hashed: HashMap<PathBuf, (SystemTime, u64, String)>,
    sources: HashMap<String, PathBuf>,
    /// What the compositor holds right now.
    held: BTreeSet<String>,
}

impl LockAssets {
    /// The `asset:<sha256>` source for the image at `path`.
    pub fn source(&mut self, path: &Path) -> Option<String> {
        let meta = std::fs::metadata(path).ok()?;
        if !meta.is_file() || meta.len() > Limits::ASSET_BYTES as u64 {
            return None;
        }
        let modified = meta.modified().ok()?;
        let id = match self.hashed.get(path) {
            Some((when, len, id)) if *when == modified && *len == meta.len() => id.clone(),
            _ => {
                let id = format!("{:x}", Sha256::digest(std::fs::read(path).ok()?));
                self.hashed.insert(path.to_owned(), (modified, meta.len(), id.clone()));
                id
            }
        };
        self.sources.insert(id.clone(), path.to_owned());
        AssetId::parse(&id).map(|id| id.to_src())
    }

    /// Bytes for each of `ids` the compositor does not hold yet.
    pub fn missing<'a>(&self, ids: impl IntoIterator<Item = &'a AssetId>) -> Vec<(String, Vec<u8>)> {
        ids.into_iter()
            .filter(|id| !self.held.contains(id.as_str()))
            .filter_map(|id| {
                let bytes = std::fs::read(self.sources.get(id.as_str())?).ok()?;
                Some((id.as_str().to_owned(), bytes))
            })
            .collect()
    }

    pub fn held(&mut self, id: String) {
        self.held.insert(id);
    }

    /// A new scene replaced the old one; the compositor keeps only what it references.
    pub fn retain(&mut self, referenced: &BTreeSet<AssetId>) {
        self.held.retain(|id| referenced.iter().any(|kept| kept.as_str() == id));
    }
}

impl LockAssets {
    /// Package-relative image paths become `asset:<sha256>` sources; an
    /// image that cannot be found drops out rather than spoil the scene.
    fn rewrite_images(&mut self, node: &mut Node, dir: Option<&Path>) {
        match node {
            Node::Image { src, .. } if !src.starts_with(ASSET_SRC_PREFIX) => {
                let resolved = dir
                    .zip(crownplugin_model::package::relative_path(src))
                    .and_then(|(dir, rel)| self.source(&dir.join(rel)));
                match resolved {
                    Some(asset) => *src = asset,
                    None => *node = Node::Spacer,
                }
            }
            Node::VStack { children, .. }
            | Node::HStack { children, .. }
            | Node::ZStack { children, .. }
            | Node::Card { children, .. } => {
                for child in children {
                    self.rewrite_images(child, dir);
                }
            }
            _ => {}
        }
    }

    /// The lock scene for the current settings, theme and plugin slots, with
    /// every image already named by its asset id.
    pub fn scene(&mut self, surfaces: &Surfaces, registry: &Registry, theme_dir: Option<&Path>) -> LockScene {
        let mut lock: Lock = crownos_config::load_strict(Lock::SECTION).unwrap_or_default();
        let theme: Theme = crownos_config::load_strict(Theme::SECTION).unwrap_or_default();
        if let LockBackground::Image(path) = &lock.background
            && !path.starts_with(ASSET_SRC_PREFIX)
        {
            lock.background = self
                .source(Path::new(path))
                .map_or(LockBackground::Desktop, LockBackground::Image);
        }
        let decoration = surfaces.lock_layout().cloned().map(|mut tree| {
            self.rewrite_images(&mut tree.root, theme_dir);
            tree
        });
        let slots: Vec<(String, LockSlotPosition, Tree)> = surfaces
            .lock()
            .surfaces
            .into_iter()
            .filter_map(|surface| {
                let mut tree = surface.main?;
                let dir = registry.get(&surface.plugin).map(|entry| entry.package.dir.clone());
                self.rewrite_images(&mut tree.root, dir.as_deref());
                Some((
                    format!("plugin:{}/{}", surface.plugin, surface.widget),
                    surface.lock_slot.unwrap_or_default(),
                    tree,
                ))
            })
            .collect();
        lockscene::compose(&lock, &theme, decoration, slots)
    }
}

#[cfg(test)]
mod tests {
    use crownplugin_proto::Part;

    use super::*;
    use crate::util::testenv::TestEnv;

    #[test]
    fn a_scene_names_every_image_by_asset_and_validates() {
        let env = TestEnv::new();
        let picture = env.root.path().join("lock.png");
        std::fs::write(&picture, b"not really a png").expect("write");
        crownos_config::write_keys(
            "lock",
            [("background", format!("Image({:?})", picture.display().to_string()).as_str())],
        )
        .expect("lock.ron");

        let mut surfaces = Surfaces::default();
        let slot = Node::VStack {
            style: Vec::new(),
            children: vec![
                Node::Text { style: Vec::new(), text: "Good morning".into() },
                Node::Image { style: Vec::new(), src: "missing.png".into() },
            ],
        };
        surfaces
            .render("dev.a.greeting", "hello", Part::Main, Some(slot), Some(LockSlotPosition::BelowClock))
            .expect("fits");

        let mut assets = LockAssets::default();
        let scene = assets.scene(&surfaces, &Registry::default(), None);
        let LockBackground::Image(background) = &scene.config.background else {
            panic!("background should stay an image: {:?}", scene.config.background);
        };
        assert!(background.starts_with(ASSET_SRC_PREFIX));
        assert_eq!(scene.slots[0].id, "plugin:dev.a.greeting/hello");
        assert_eq!(scene.referenced_assets().len(), 1);
        assert_eq!(assets.missing(&scene.referenced_assets()).len(), 1);
        lockscene::validate(scene).expect("a valid scene");
    }
}
