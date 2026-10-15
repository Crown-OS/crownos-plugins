//! The real daemon, driven over its socket in throwaway directories.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use crownplugin_proto::{Host, SettingValue, Source, plugind};

const TIMEOUT: Option<Duration> = Some(Duration::from_secs(20));

struct Daemon {
    child: Child,
    root: tempfile::TempDir,
    client: plugind::Client,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Daemon {
    fn start() -> Self {
        let root = tempfile::Builder::new().prefix("cp").tempdir_in("/tmp").expect("tempdir");
        let dir = |name: &str| root.path().join(name);
        std::fs::create_dir_all(dir("run")).expect("runtime dir");
        let child = Command::new(env!("CARGO_BIN_EXE_crownplugind"))
            .env("XDG_RUNTIME_DIR", dir("run"))
            .env("CROWN_CONFIG_DIR", dir("config"))
            .env("CROWN_DATA_DIR", dir("data"))
            .env("CROWN_STATE_DIR", dir("state"))
            .env("XDG_CONFIG_HOME", dir("xdg-config"))
            .env("XDG_DATA_HOME", dir("xdg-data"))
            .env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}", dir("no-bus").display()))
            .env("RUST_LOG", "warn")
            .env_remove("WAYLAND_DISPLAY")
            .spawn()
            .expect("daemon starts");
        let sockets = dir("run").join("crownos");
        let started = Instant::now();
        let client = loop {
            match plugind::Client::connect_in(&sockets) {
                Ok(client) => break client,
                Err(_) if started.elapsed() < Duration::from_secs(10) => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(error) => panic!("daemon never listened: {error}"),
            }
        };
        Self { child, root, client }
    }

    fn config(&self, section: &str) -> String {
        std::fs::read_to_string(self.root.path().join("config").join(format!("{section}.ron"))).unwrap_or_default()
    }
}

macro_rules! call {
    ($daemon:expr, $method:ident($($arg:expr),*)) => {{
        let pending = $daemon.client.$method($($arg),*).expect("sent");
        $daemon.client.wait(pending, TIMEOUT)
    }};
}

fn example(name: &str) -> Source {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples").join(name);
    Source::Path(path.canonicalize().expect("example").to_string_lossy().into_owned())
}

fn eventually(mut check: impl FnMut() -> bool) {
    let started = Instant::now();
    while !check() {
        assert!(started.elapsed() < Duration::from_secs(10), "condition never held");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_plugin_installs_draws_takes_settings_and_disables_cleanly() {
    let mut daemon = Daemon::start();
    let bar_before = daemon.config("bar");

    let id = call!(daemon, install(example("cpu-graph"))).expect("installs");
    assert_eq!(id, "crown.example.cpu-graph");
    call!(daemon, set_enabled(id.clone(), true)).expect("enables");
    assert!(daemon.config("bar").contains("plugin:crown.example.cpu-graph/status"));

    eventually(|| {
        let snapshot = call!(daemon, attach(Host::Bar)).expect("attaches");
        snapshot.surfaces.iter().any(|surface| surface.widget == "status" && surface.main.is_some())
    });

    call!(daemon, set_setting(id.clone(), "interval".into(), SettingValue::Number(4.0))).expect("valid setting");
    let rejected = call!(daemon, set_setting(id.clone(), "interval".into(), SettingValue::Number(99.0)));
    assert!(rejected.is_err(), "out of the slider's range");
    let values = call!(daemon, settings(id.clone())).expect("settings");
    assert!(values.contains(&("interval".into(), SettingValue::Number(4.0))));

    call!(daemon, set_enabled(id.clone(), false)).expect("disables");
    let snapshot = call!(daemon, attach(Host::Bar)).expect("attaches");
    assert!(snapshot.surfaces.is_empty());
    let bar_after = daemon.config("bar");
    assert!(!bar_after.contains("plugin:"), "{bar_after}");
    if !bar_before.is_empty() {
        assert_eq!(bar_after, bar_before);
    }
}

#[test]
fn a_theme_applies_and_the_stock_look_comes_back() {
    let mut daemon = Daemon::start();
    let id = call!(daemon, install(example("midnight-theme"))).expect("installs");
    call!(daemon, set_theme(Some(id.clone()))).expect("activates");
    assert!(daemon.config("appearance").contains("blur_passes: 4"));
    assert!(daemon.config("theme").contains("#0b0d12"));
    assert!(daemon.config("lock").contains("clock_style: Large"));

    call!(daemon, set_theme(None)).expect("deactivates");
    assert!(daemon.config("appearance").contains("blur_passes: 3"));
    assert!(!daemon.config("theme").contains("#0b0d12"));
    let listed = call!(daemon, list()).expect("lists");
    assert!(listed.iter().any(|entity| entity.id == id && !entity.active));
}

#[test]
fn an_invalid_package_is_refused_with_its_errors() {
    let mut daemon = Daemon::start();
    let broken = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        broken.path().join("plugin.ron"),
        r#"Plugin(id: "dev.test.broken", name: "Broken", version: "1.0.0", contributes: (config: {"plugins": {"enabled": []}}))"#,
    )
    .expect("write");
    std::fs::write(broken.path().join("main.luau"), "return {}").expect("write");
    let error = call!(daemon, install(Source::Path(broken.path().to_string_lossy().into_owned())))
        .expect_err("refused");
    assert!(error.to_string().contains("config.plugins.enabled"), "{error}");
}
