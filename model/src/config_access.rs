//! Which capability writing a given config key needs.

use crate::Capability;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigAccess {
    /// Look-only: allowed under [`Capability::ThemeVisual`], which is safe.
    Visual,
    Needs(Capability),
    /// Never writable by a theme or plugin.
    Forbidden,
}

pub fn config_access(section: &str, key: &str) -> ConfigAccess {
    match (section, key) {
        ("appearance" | "theme" | "bar" | "lock", _) => ConfigAccess::Visual,
        ("plugins", _) => ConfigAccess::Forbidden,
        ("keybinds", _) => ConfigAccess::Needs(Capability::Keybinds),
        ("power", _) => ConfigAccess::Needs(Capability::PowerConfig),
        ("display", _) => ConfigAccess::Needs(Capability::DisplayConfig),
        ("compositor", "startup") => ConfigAccess::Needs(Capability::StartupConfig),
        ("compositor", "window_rules") => ConfigAccess::Needs(Capability::WindowRules),
        (section, _) if crownos_config::schema::SECTIONS.contains(&section) => {
            ConfigAccess::Needs(Capability::Config {
                section: section.to_owned(),
            })
        }
        _ => ConfigAccess::Forbidden,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_is_dangerous_and_plugins_is_off_limits() {
        assert_eq!(
            config_access("compositor", "startup"),
            ConfigAccess::Needs(Capability::StartupConfig)
        );
        assert_eq!(config_access("plugins", "enabled"), ConfigAccess::Forbidden);
        assert_eq!(config_access("appearance", "blur_passes"), ConfigAccess::Visual);
        assert_eq!(
            config_access("input", "natural_scroll"),
            ConfigAccess::Needs(Capability::Config { section: "input".into() })
        );
        assert_eq!(config_access("nope", "x"), ConfigAccess::Forbidden);
    }
}
