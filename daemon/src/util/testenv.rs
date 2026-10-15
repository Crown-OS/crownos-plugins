//! Isolated config, data and state directories for tests that touch disk.

use std::sync::{Mutex, MutexGuard};

use crownplugin_model::package::{DATA_DIR_ENV, STATE_DIR_ENV};

static LOCK: Mutex<()> = Mutex::new(());

pub struct TestEnv {
    pub root: tempfile::TempDir,
    _guard: MutexGuard<'static, ()>,
}

impl TestEnv {
    pub fn new() -> Self {
        let guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = tempfile::tempdir().expect("tempdir");
        // SAFETY: every test that reads these variables holds `LOCK`.
        unsafe {
            std::env::set_var(crownos_config::CONFIG_DIR_ENV, root.path().join("config"));
            std::env::set_var(DATA_DIR_ENV, root.path().join("data"));
            std::env::set_var(STATE_DIR_ENV, root.path().join("state"));
        }
        Self { root, _guard: guard }
    }

    pub fn config(&self, section: &str) -> String {
        std::fs::read_to_string(crownos_config::path_for(section)).unwrap_or_default()
    }
}
