//! Everything a package can get wrong, reported with a path to the offending
//! field so a person or an AI can fix it without guessing.

use std::collections::BTreeSet;
use std::fmt;

use crownos_config::schema::LockBackground;
use crownos_config::schema::theme::{is_color_token, parse_hex};

use crate::manifest::ConfigPatch;
use crate::{
    API_VERSION, Capability, ConfigAccess, Field, Manifest, Package, PluginManifest,
    ThemeManifest, config_access,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Dotted path into the manifest, e.g. `contributes.bar[0].id`.
    pub at: String,
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.at, self.message)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub errors: Vec<Issue>,
    pub warnings: Vec<Issue>,
}

impl Report {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    fn error(&mut self, at: impl Into<String>, message: impl Into<String>) {
        self.errors.push(Issue {
            at: at.into(),
            message: message.into(),
        });
    }

    fn warn(&mut self, at: impl Into<String>, message: impl Into<String>) {
        self.warnings.push(Issue {
            at: at.into(),
            message: message.into(),
        });
    }
}

pub fn validate(package: &Package) -> Report {
    let mut report = Report::default();
    let manifest = &package.manifest;
    if manifest.api() != API_VERSION {
        report.error(
            "api",
            format!("this CrownOS understands api {API_VERSION}, not {}", manifest.api()),
        );
    }
    if manifest.name().trim().is_empty() {
        report.error("name", "must not be empty");
    }
    if manifest.version().split('.').count() != 3
        || manifest.version().split('.').any(|part| part.parse::<u32>().is_err())
    {
        report.error("version", "must be semver like `1.0.0`");
    }
    if let Some(script) = manifest.script() {
        check_file(package, &mut report, "script", script);
    }
    let declared = manifest.capabilities();
    check_config(&mut report, "config", manifest.config(), &declared);
    match manifest {
        Manifest::Theme(theme) => check_theme(package, &mut report, theme),
        Manifest::Plugin(plugin) => check_plugin(&mut report, plugin),
    }
    report
}

fn check_file(package: &Package, report: &mut Report, at: &str, rel: &str) {
    match package.resolve(rel) {
        None => report.error(at, format!("`{rel}` points outside the package")),
        Some(path) if !path.exists() => report.error(at, format!("`{rel}` does not exist")),
        Some(_) => {}
    }
}

fn check_config(report: &mut Report, at: &str, patch: &ConfigPatch, declared: &[Capability]) {
    for (section, keys) in patch {
        for (key, value) in keys {
            let at = format!("{at}.{section}.{key}");
            match config_access(section, key) {
                ConfigAccess::Forbidden => {
                    report.error(&at, format!("`{section}` cannot be changed by a package"));
                    continue;
                }
                ConfigAccess::Needs(capability) if !declared.contains(&capability) => {
                    report.error(
                        &at,
                        format!("needs `{capability:?}` in capabilities"),
                    );
                }
                _ => {}
            }
            if let Err(error) = crownos_config::schema::check_value(section, key, value.as_str()) {
                report.error(&at, error.to_string());
            }
        }
    }
}

fn check_colors(
    report: &mut Report,
    at: &str,
    colors: &std::collections::BTreeMap<String, String>,
) {
    for (token, hex) in colors {
        if !is_color_token(token) {
            report.error(format!("{at}.{token}"), "not a theme color token");
        }
        if parse_hex(hex).is_none() {
            report.error(format!("{at}.{token}"), format!("`{hex}` is not #rrggbb or #rrggbbaa"));
        }
    }
}

fn check_theme(package: &Package, report: &mut Report, theme: &ThemeManifest) {
    check_colors(report, "tokens.light", &theme.tokens.light);
    check_colors(report, "tokens.dark", &theme.tokens.dark);
    if let Some(accent) = &theme.tokens.accent
        && (parse_hex(&accent.start).is_none() || parse_hex(&accent.end).is_none())
    {
        report.error("tokens.accent", "start and end must be #rrggbb or #rrggbbaa");
    }
    if let Some(wallpaper) = &theme.wallpaper {
        for (mode, path) in [("light", &wallpaper.light), ("dark", &wallpaper.dark)] {
            if let Some(path) = path {
                check_file(package, report, &format!("wallpaper.{mode}"), path);
            }
        }
    }
    let assets = &theme.assets;
    for (name, dir) in [
        ("icons", &assets.icons),
        ("cursors", &assets.cursors),
        ("sounds", &assets.sounds),
        ("system", &assets.system),
    ] {
        if let Some(dir) = dir {
            check_file(package, report, &format!("assets.{name}"), dir);
        }
    }
    for (index, font) in assets.fonts.iter().enumerate() {
        check_file(package, report, &format!("assets.fonts[{index}]"), font);
    }
    if let Some(lock) = &theme.lock {
        match &lock.background {
            Some(LockBackground::Image(path)) => check_file(package, report, "lock.background", path),
            Some(LockBackground::Token(token)) if !is_color_token(token) => {
                report.error("lock.background", format!("`{token}` is not a theme color token"));
            }
            _ => {}
        }
        if lock.dim.is_some_and(|dim| !(0.0..=1.0).contains(&dim)) {
            report.error("lock.dim", "must be between 0.0 and 1.0");
        }
        if lock.font.is_some() && assets.system.is_none() {
            report.warn(
                "lock.font",
                "the lock screen only sees system fonts; ship the font under `assets.system`",
            );
        }
    }
}

fn check_plugin(report: &mut Report, plugin: &PluginManifest) {
    let contributes = &plugin.contributes;
    let mut widget_ids = BTreeSet::new();
    for (index, widget) in contributes.bar.iter().enumerate() {
        if !is_plain_id(&widget.id) {
            report.error(format!("contributes.bar[{index}].id"), "use a-z, 0-9 and '-'");
        }
        if !widget_ids.insert(widget.id.as_str()) {
            report.error(format!("contributes.bar[{index}].id"), "duplicate widget id");
        }
    }
    let mut slot_ids = BTreeSet::new();
    for (index, slot) in contributes.lock.iter().enumerate() {
        if !is_plain_id(&slot.id) || !slot_ids.insert(slot.id.as_str()) {
            report.error(format!("contributes.lock[{index}].id"), "must be unique, a-z, 0-9 and '-'");
        }
    }
    for (index, keybind) in contributes.keybinds.iter().enumerate() {
        if keybind.keys.trim().is_empty() {
            report.error(format!("contributes.keybinds[{index}].keys"), "must not be empty");
        }
        if !is_plain_id(&keybind.action) {
            report.error(
                format!("contributes.keybinds[{index}].action"),
                "names a handler in `actions`; use a-z, 0-9 and '-'",
            );
        }
    }
    if let Some(settings) = &contributes.settings {
        let mut keys = BTreeSet::new();
        for (index, field) in settings.fields.iter().enumerate() {
            let at = format!("contributes.settings.fields[{index}]");
            if !keys.insert(field.key()) {
                report.error(&at, format!("duplicate key `{}`", field.key()));
            }
            if !field.accepts(&field.default_value()) {
                report.error(&at, "default does not fit the field (range, options or token)");
            }
            if let Field::Slider { step, .. } = field
                && *step <= 0.0
            {
                report.error(&at, "step must be positive");
            }
        }
    }
    for (index, capability) in plugin.capabilities.iter().enumerate() {
        match capability {
            Capability::Network { hosts } if hosts.is_empty() => report.error(
                format!("capabilities[{index}]"),
                "list the hosts the plugin talks to",
            ),
            Capability::Exec { programs } if programs.is_empty() => report.error(
                format!("capabilities[{index}]"),
                "list the programs the plugin runs",
            ),
            _ => {}
        }
    }
    if plugin.limits.memory_mb == 0 || plugin.limits.memory_mb > 256 {
        report.error("limits.memory_mb", "must be 1 to 256");
    }
    if plugin.limits.call_budget_ms == 0 || plugin.limits.call_budget_ms > 1000 {
        report.error("limits.call_budget_ms", "must be 1 to 1000");
    }
}

fn is_plain_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
