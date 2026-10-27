use std::path::Path;

use crownplugin_model::{Package, validate};

fn write(dir: &Path, name: &str, text: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(path, text).expect("write");
}

fn package(files: &[(&str, &str)]) -> (tempfile::TempDir, Package) {
    let dir = tempfile::tempdir().expect("tempdir");
    for (name, text) in files {
        write(dir.path(), name, text);
    }
    let package = Package::load(dir.path()).expect("manifest parses");
    (dir, package)
}

fn errors(package: &Package) -> Vec<String> {
    validate(package).errors.iter().map(ToString::to_string).collect()
}

#[test]
fn a_complete_theme_validates() {
    let (_dir, package) = package(&[
        (
            "theme.ron",
            r##"Theme(
                id: "crown.midnight", name: "Midnight", version: "1.0.0",
                tokens: (dark: {"surface.bg": "#0b0d12"}, accent: (start: "#8f6dfb", end: "#6d48e8")),
                config: {"appearance": {"blur_passes": 4, "accent": Blue}, "bar": {"floating": true}},
                wallpaper: (dark: "night.jpg"),
                lock: (background: Image("lock.jpg"), clock_style: Large, dim: 0.4),
                assets: (system: "system", gtk: true),
            )"##,
        ),
        ("night.jpg", ""),
        ("lock.jpg", ""),
        ("system/fonts/Orbitron.ttf", ""),
    ]);
    assert_eq!(errors(&package), Vec::<String>::new());
}

#[test]
fn bad_tokens_values_and_missing_files_are_all_reported() {
    let (_dir, package) = package(&[(
        "theme.ron",
        r##"Theme(
            id: "crown.broken", name: "Broken", version: "1.0",
            tokens: (dark: {"surface.nope": "#000000", "text.body": "red"}),
            config: {"appearance": {"blur_passes": "four"}, "plugins": {"enabled": []}},
            wallpaper: (dark: "../escape.jpg"),
        )"##,
    )]);
    let errors = errors(&package);
    for needle in [
        "version",
        "tokens.dark.surface.nope",
        "tokens.dark.text.body",
        "config.appearance.blur_passes",
        "config.plugins.enabled",
        "wallpaper.dark",
    ] {
        assert!(
            errors.iter().any(|error| error.starts_with(needle)),
            "expected an error at {needle}, got {errors:#?}"
        );
    }
}

#[test]
fn lock_transitions_are_held_to_the_scene_bounds() {
    let (_dir, package) = package(&[(
        "theme.ron",
        r#"Theme(
            id: "crown.restless", name: "Restless", version: "1.0.0",
            lock: (transitions: (
                appear: (effects: [Fade, Zoom(from: 0.9)], duration_ms: 400),
                reveal: (effects: [Blur(radius: 500.0), Slide(from: Top, distance: 40.0)]),
                dismiss: (effects: [Zoom(from: 3.0)], duration_ms: 60000),
            )),
        )"#,
    )]);
    let errors = errors(&package);
    for needle in [
        "lock.transitions.reveal.effects[0]",
        "lock.transitions.dismiss.effects[0]",
        "lock.transitions.dismiss.duration_ms",
    ] {
        assert!(
            errors.iter().any(|error| error.starts_with(needle)),
            "expected an error at {needle}, got {errors:#?}"
        );
    }
    assert_eq!(errors.len(), 3, "{errors:#?}");
}

#[test]
fn a_plugin_must_declare_what_its_config_needs() {
    let (_dir, package) = package(&[
        (
            "plugin.ron",
            r#"Plugin(
                id: "dev.test.startup", name: "Startup", version: "1.0.0",
                contributes: (config: {"compositor": {"startup": ["foo"]}}),
            )"#,
        ),
        ("main.luau", "return {}"),
    ]);
    assert!(errors(&package)
        .iter()
        .any(|error| error.contains("StartupConfig")));
}

#[test]
fn plugin_settings_defaults_must_fit_their_field() {
    let (_dir, package) = package(&[
        (
            "plugin.ron",
            r#"Plugin(
                id: "dev.test.cpu", name: "CPU", version: "0.1.0",
                capabilities: [Feed(System)],
                contributes: (
                    bar: [(id: "graph"), (id: "graph")],
                    settings: (title: "CPU", fields: [
                        Slider(key: "interval", label: "Interval", min: 1.0, max: 5.0, default: 9.0),
                        Select(key: "style", label: "Style", options: [(value: "bars", label: "Bars")], default: "bars"),
                    ]),
                ),
            )"#,
        ),
        ("main.luau", "return {}"),
    ]);
    let errors = errors(&package);
    assert!(errors.iter().any(|error| error.starts_with("contributes.bar[1].id")));
    assert!(errors
        .iter()
        .any(|error| error.starts_with("contributes.settings.fields[0]")));
    assert!(!errors
        .iter()
        .any(|error| error.starts_with("contributes.settings.fields[1]")));
}

#[test]
fn a_missing_entry_script_is_an_error() {
    let (_dir, package) = package(&[(
        "plugin.ron",
        r#"Plugin(id: "dev.test.none", name: "None", version: "0.1.0")"#,
    )]);
    assert!(errors(&package).iter().any(|error| error.starts_with("script")));
}
