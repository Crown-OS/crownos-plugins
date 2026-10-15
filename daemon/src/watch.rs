//! Edits made outside the daemon: `plugins.ron` by hand, theme and
//! appearance changes from Settings, plugin settings, dev package folders.

use std::collections::HashMap;
use std::path::Path;

use calloop::channel::Sender;
use crownos_config::Subscription;
use crownos_config::schema::Plugins;
use crownplugin_model::EntityId;
use notify::{RecursiveMode, Watcher};

use crate::daemon::Message;

pub struct Watches {
    sender: Sender<Message>,
    _config: Vec<Subscription>,
    dev: HashMap<String, notify::RecommendedWatcher>,
}

impl Watches {
    pub fn start(sender: Sender<Message>) -> Self {
        let plugins = {
            let sender = sender.clone();
            crownos_config::subscribe_typed::<Plugins, _>(Plugins::SECTION, move |plugins| {
                let _ = sender.send(Message::PluginsEdited(plugins));
            })
        };
        let theme_watch = |section: &'static str| {
            let sender = sender.clone();
            crownos_config::subscribe(section, move |_| {
                let _ = sender.send(Message::ThemeChanged);
            })
        };
        Self {
            _config: vec![
                plugins,
                theme_watch("theme"),
                theme_watch("appearance"),
                theme_watch("lock"),
            ],
            dev: HashMap::new(),
            sender,
        }
    }

    pub fn settings(&self, id: &EntityId) -> Option<Subscription> {
        let sender = self.sender.clone();
        let owned = id.to_string();
        Some(crownos_config::subscribe(&id.settings_section(), move |_| {
            let _ = sender.send(Message::SettingsEdited(owned.clone()));
        }))
    }

    pub fn dev(&mut self, id: &str, dir: &Path) {
        let sender = self.sender.clone();
        let owned = id.to_owned();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok_and(|event| event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove()) {
                let _ = sender.send(Message::DevChanged(owned.clone()));
            }
        });
        match watcher {
            Ok(mut watcher) => match watcher.watch(dir, RecursiveMode::Recursive) {
                Ok(()) => {
                    self.dev.insert(id.to_owned(), watcher);
                }
                Err(error) => tracing::warn!("cannot watch {}: {error}", dir.display()),
            },
            Err(error) => tracing::warn!("cannot watch {}: {error}", dir.display()),
        }
    }

    pub fn forget_dev(&mut self, id: &str) {
        self.dev.remove(id);
    }
}
