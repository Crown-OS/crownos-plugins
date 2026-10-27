//! What enabling a theme or plugin changes, as journal [`Change`]s.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use crownos_config::schema::{Bar, Binding, Keybinds, LockBackground, Theme};
use crownplugin_model::{Package, PluginManifest, ThemeManifest};
use serde::Serialize;

use crate::journal::Change;
use crate::theme::{gtk, qt};

fn ron<T: Serialize>(value: &T) -> Result<String> {
    ron::to_string(value).context("serializing a config value")
}

fn set(section: &str, key: &str, value: String) -> Change {
    Change::Set {
        section: section.to_owned(),
        key: key.to_owned(),
        value,
    }
}

fn config_changes(patch: &crownplugin_model::ConfigPatch) -> impl Iterator<Item = Change> + '_ {
    patch.iter().flat_map(|(section, keys)| {
        keys.iter()
            .map(move |(key, value)| set(section, key, value.as_str().to_owned()))
    })
}

fn home_dir(sub: &str) -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join(sub)
}

fn dir_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Whether the theme, once applied, will be in dark mode.
pub fn theme_dark_mode(theme: &ThemeManifest) -> bool {
    theme
        .config
        .get("appearance")
        .and_then(|keys| keys.get("dark_mode"))
        .and_then(|value| value.as_str().parse::<bool>().ok())
        .unwrap_or_else(|| crownos_config::load::<crownos_config::Appearance>("appearance").dark_mode)
}

pub fn theme_changes(package: &Package, theme: &ThemeManifest) -> Result<Vec<Change>> {
    let mut changes: Vec<Change> = config_changes(&theme.config).collect();
    let tokens = &theme.tokens;
    for (key, colors) in [("light", &tokens.light), ("dark", &tokens.dark)] {
        for (token, hex) in colors {
            changes.push(Change::MapSet {
                section: Theme::SECTION.to_owned(),
                key: key.to_owned(),
                entry: token.clone(),
                value: ron(hex)?,
            });
        }
    }
    if tokens.accent.is_some() {
        changes.push(set(Theme::SECTION, "accent", ron(&tokens.accent)?));
    }
    if let Some(radii) = &tokens.radii {
        changes.push(set(Theme::SECTION, "radii", ron(radii)?));
    }
    if let Some(font) = &tokens.font {
        changes.push(set(Theme::SECTION, "font", ron(font)?));
    }

    let dark_mode = theme_dark_mode(theme);
    if let Some(path) = theme
        .wallpaper
        .as_ref()
        .and_then(|wallpaper| wallpaper.for_mode(dark_mode))
        .and_then(|rel| package.resolve(rel))
    {
        changes.push(set("appearance", "wallpaper", ron(&path.to_string_lossy())?));
    }

    if let Some(lock) = &theme.lock {
        if let Some(background) = &lock.background {
            let background = match background {
                LockBackground::Image(rel) => LockBackground::Image(
                    package
                        .resolve(rel)
                        .map(|path| path.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                ),
                other => other.clone(),
            };
            changes.push(set("lock", "background", ron(&background)?));
        }
        let optional: [(&str, Option<String>); 10] = [
            ("clock_style", lock.clock_style.map(|v| ron(&v)).transpose()?),
            ("time_format", lock.time_format.as_ref().map(ron).transpose()?),
            ("date_format", lock.date_format.as_ref().map(ron).transpose()?),
            ("font", lock.font.as_ref().map(|font| ron(&Some(font))).transpose()?),
            ("layout", lock.layout.map(|v| ron(&v)).transpose()?),
            ("dim", lock.dim.map(|v| ron(&v)).transpose()?),
            ("blur", lock.blur.map(|v| ron(&v)).transpose()?),
            ("auth_field", lock.auth_field.map(|v| ron(&v)).transpose()?),
            ("transitions", lock.transitions.as_ref().map(ron).transpose()?),
            ("show_user", lock.show_user.map(|v| ron(&v)).transpose()?),
        ];
        changes.extend(
            optional
                .into_iter()
                .filter_map(|(key, value)| value.map(|value| set("lock", key, value))),
        );
    }

    let assets = &theme.assets;
    if let Some(icons) = assets.icons.as_deref().and_then(|rel| package.resolve(rel)) {
        let name = dir_name(&icons);
        changes.push(Change::Link {
            path: home_dir("icons").join(&name),
            target: icons,
        });
        changes.push(set("appearance", "icon_theme", ron(&name)?));
    }
    if let Some(cursors) = assets.cursors.as_deref().and_then(|rel| package.resolve(rel)) {
        let name = dir_name(&cursors);
        changes.push(Change::Link {
            path: home_dir("icons").join(&name),
            target: cursors,
        });
        changes.push(set("appearance", "cursor_theme", ron(&name)?));
    }
    for (index, font) in assets.fonts.iter().enumerate() {
        if let Some(target) = package.resolve(font) {
            changes.push(Change::Link {
                path: home_dir("fonts").join(format!("crownos-{}-{index}", theme.id)),
                target,
            });
        }
    }
    if let Some(sounds) = assets.sounds.as_deref().and_then(|rel| package.resolve(rel)) {
        changes.push(Change::Link {
            path: home_dir("sounds").join(dir_name(&sounds)),
            target: sounds,
        });
    }
    if assets.system.is_some() || assets.cursors.is_some() {
        changes.push(Change::System);
    }
    let palette = gtk::Palette::of(theme, dark_mode);
    let config = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    if assets.gtk {
        let css = gtk::css(&palette);
        for version in ["gtk-3.0", "gtk-4.0"] {
            changes.push(Change::File {
                path: config.join(version).join("gtk.css"),
                contents: css.clone().into_bytes(),
            });
        }
    }
    if assets.qt {
        let scheme = qt::color_scheme(&palette);
        for tool in ["qt5ct", "qt6ct"] {
            changes.push(Change::File {
                path: config.join(tool).join("colors").join("crownos.conf"),
                contents: scheme.clone().into_bytes(),
            });
        }
    }
    Ok(changes)
}

fn same_chord(a: &str, b: &str) -> bool {
    let normalize = |chord: &str| {
        let mut parts: Vec<String> = chord
            .split('+')
            .map(|part| part.trim().to_ascii_lowercase())
            .collect();
        parts.sort();
        parts
    };
    normalize(a) == normalize(b)
}

/// The plugin's changes, plus warnings about contributions that were skipped.
pub fn plugin_changes(plugin: &PluginManifest) -> Result<(Vec<Change>, Vec<String>)> {
    let mut changes: Vec<Change> = config_changes(&plugin.contributes.config).collect();
    let mut warnings = Vec::new();
    let bar: Bar = crownos_config::load_strict(Bar::SECTION).unwrap_or_default();
    for widget in &plugin.contributes.bar {
        let entry = ron(&widget.entry(&plugin.id))?;
        let replaced = widget.replaces.as_deref().and_then(|builtin| {
            [("left", &bar.left), ("center", &bar.center), ("right", &bar.right)]
                .into_iter()
                .find(|(_, items)| items.iter().any(|item| item == builtin))
                .map(|(key, _)| (key, builtin))
        });
        changes.push(match replaced {
            Some((key, builtin)) => Change::Replace {
                section: Bar::SECTION.to_owned(),
                key: key.to_owned(),
                old: ron(&builtin)?,
                new: entry,
            },
            None => Change::Insert {
                section: Bar::SECTION.to_owned(),
                key: widget.slot.config_key().to_owned(),
                item: entry,
                after: widget.after.as_ref().map(ron).transpose()?,
            },
        });
    }
    for rule in &plugin.contributes.window_rules {
        changes.push(Change::Insert {
            section: "compositor".to_owned(),
            key: "window_rules".to_owned(),
            item: ron(rule)?,
            after: None,
        });
    }
    let existing: Keybinds = crownos_config::load_strict(Keybinds::SECTION).unwrap_or_default();
    for keybind in &plugin.contributes.keybinds {
        let action = keybind.compositor_action(&plugin.id);
        let taken = existing
            .custom_keybinds
            .iter()
            .any(|binding| same_chord(&binding.keys, &keybind.keys) && binding.action != action);
        if taken {
            warnings.push(format!("{} is already bound; not taking it over", keybind.keys));
            continue;
        }
        changes.push(Change::Insert {
            section: Keybinds::SECTION.to_owned(),
            key: "custom_keybinds".to_owned(),
            item: ron(&Binding {
                keys: keybind.keys.clone(),
                action,
            })?,
            after: None,
        });
    }
    Ok((changes, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_compare_without_order_or_case() {
        assert!(same_chord("Super+Shift+C", "shift + super + c"));
        assert!(!same_chord("Super+C", "Super+Shift+C"));
    }
}
