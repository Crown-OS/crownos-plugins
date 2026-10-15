//! The settings page a plugin declares; Settings renders it and stores the
//! values in the plugin's own `plugin.<id>.ron`.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsSection {
    pub title: String,
    /// A Lucide icon name.
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub description: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Field {
    Toggle {
        key: String,
        label: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        default: bool,
    },
    Slider {
        key: String,
        label: String,
        #[serde(default)]
        description: String,
        min: f64,
        max: f64,
        #[serde(default = "unit_step")]
        step: f64,
        #[serde(default)]
        unit: String,
        default: f64,
    },
    Select {
        key: String,
        label: String,
        #[serde(default)]
        description: String,
        options: Vec<SelectOption>,
        default: String,
    },
    /// A theme color token such as `accent.start`.
    ColorToken {
        key: String,
        label: String,
        #[serde(default)]
        description: String,
        default: String,
    },
    Text {
        key: String,
        label: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        secret: bool,
        #[serde(default)]
        default: String,
    },
    File {
        key: String,
        label: String,
        #[serde(default)]
        description: String,
        /// Extensions without the dot.
        #[serde(default)]
        filter: Vec<String>,
        #[serde(default)]
        default: String,
    },
}

fn unit_step() -> f64 {
    1.0
}

impl Field {
    pub fn key(&self) -> &str {
        match self {
            Self::Toggle { key, .. }
            | Self::Slider { key, .. }
            | Self::Select { key, .. }
            | Self::ColorToken { key, .. }
            | Self::Text { key, .. }
            | Self::File { key, .. } => key,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Toggle { label, .. }
            | Self::Slider { label, .. }
            | Self::Select { label, .. }
            | Self::ColorToken { label, .. }
            | Self::Text { label, .. }
            | Self::File { label, .. } => label,
        }
    }

    pub fn default_value(&self) -> SettingValue {
        match self {
            Self::Toggle { default, .. } => SettingValue::Bool(*default),
            Self::Slider { default, .. } => SettingValue::Number(*default),
            Self::Select { default, .. }
            | Self::ColorToken { default, .. }
            | Self::Text { default, .. }
            | Self::File { default, .. } => SettingValue::Text(default.clone()),
        }
    }

    /// Whether `value` is something this field can hold.
    pub fn accepts(&self, value: &SettingValue) -> bool {
        match (self, value) {
            (Self::Toggle { .. }, SettingValue::Bool(_)) => true,
            (Self::Slider { min, max, .. }, SettingValue::Number(number)) => {
                (*min..=*max).contains(number)
            }
            (Self::Select { options, .. }, SettingValue::Text(text)) => {
                options.iter().any(|option| &option.value == text)
            }
            (Self::ColorToken { .. }, SettingValue::Text(text)) => {
                crownos_config::schema::theme::is_color_token(text)
            }
            (Self::Text { .. } | Self::File { .. }, SettingValue::Text(_)) => true,
            _ => false,
        }
    }
}

impl SettingsSection {
    pub fn field(&self, key: &str) -> Option<&Field> {
        self.fields.iter().find(|field| field.key() == key)
    }

    /// Every field at its default, overlaid with `stored` values that still fit.
    pub fn resolve(&self, stored: &BTreeMap<String, SettingValue>) -> BTreeMap<String, SettingValue> {
        self.fields
            .iter()
            .map(|field| {
                let value = stored
                    .get(field.key())
                    .filter(|value| field.accepts(value))
                    .cloned()
                    .unwrap_or_else(|| field.default_value());
                (field.key().to_owned(), value)
            })
            .collect()
    }
}

/// A stored setting. Written as a bare `true` / `0.5` / `"text"` in RON so
/// the file stays hand-editable, and as a tagged enum on the wire.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingValue {
    Bool(bool),
    Number(f64),
    Text(String),
}

#[derive(Serialize, Deserialize)]
enum Tagged {
    Bool(bool),
    Number(f64),
    Text(String),
}

impl Serialize for SettingValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            return match self {
                Self::Bool(value) => serializer.serialize_bool(*value),
                Self::Number(value) => serializer.serialize_f64(*value),
                Self::Text(value) => serializer.serialize_str(value),
            };
        }
        match self {
            Self::Bool(value) => Tagged::Bool(*value),
            Self::Number(value) => Tagged::Number(*value),
            Self::Text(value) => Tagged::Text(value.clone()),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SettingValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            return deserializer.deserialize_any(BareVisitor);
        }
        Ok(match Tagged::deserialize(deserializer)? {
            Tagged::Bool(value) => Self::Bool(value),
            Tagged::Number(value) => Self::Number(value),
            Tagged::Text(value) => Self::Text(value),
        })
    }
}

struct BareVisitor;

impl Visitor<'_> for BareVisitor {
    type Value = SettingValue;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a bool, a number or a string")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<SettingValue, E> {
        Ok(SettingValue::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<SettingValue, E> {
        Ok(SettingValue::Number(value as f64))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<SettingValue, E> {
        Ok(SettingValue::Number(value as f64))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<SettingValue, E> {
        Ok(SettingValue::Number(value))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<SettingValue, E> {
        Ok(SettingValue::Text(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<SettingValue, E> {
        Ok(SettingValue::Text(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slider() -> Field {
        Field::Slider {
            key: "interval".into(),
            label: "Interval".into(),
            description: String::new(),
            min: 1.0,
            max: 10.0,
            step: 1.0,
            unit: "s".into(),
            default: 2.0,
        }
    }

    #[test]
    fn stored_values_are_bare_in_ron() {
        let map = BTreeMap::from([
            ("on".to_owned(), SettingValue::Bool(true)),
            ("n".to_owned(), SettingValue::Number(2.5)),
            ("s".to_owned(), SettingValue::Text("hi".into())),
        ]);
        let text = ron::to_string(&map).expect("serializes");
        assert_eq!(text, r#"{"n":2.5,"on":true,"s":"hi"}"#);
        let back: BTreeMap<String, SettingValue> = ron::from_str(&text).expect("parses");
        assert_eq!(back, map);
        let integer: BTreeMap<String, SettingValue> = ron::from_str(r#"{"n": 3}"#).expect("parses");
        assert_eq!(integer["n"], SettingValue::Number(3.0));
    }

    #[test]
    fn out_of_range_or_wrong_kind_values_fall_back_to_the_default() {
        let section = SettingsSection {
            title: "CPU".into(),
            icon: String::new(),
            description: String::new(),
            fields: vec![slider()],
        };
        let stored = BTreeMap::from([("interval".to_owned(), SettingValue::Number(99.0))]);
        assert_eq!(section.resolve(&stored)["interval"], SettingValue::Number(2.0));
        assert!(!slider().accepts(&SettingValue::Bool(true)));
    }
}
