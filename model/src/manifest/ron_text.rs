use std::fmt;

use ron::value::RawValue;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A value kept as its RON source, so it can be checked against the real
/// type of the config key it targets instead of a lossy generic value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RonText(String);

impl RonText {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RonText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RonText {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Box::<RawValue>::deserialize(deserializer)?;
        Ok(Self(raw.trim().get_ron().to_owned()))
    }
}

impl Serialize for RonText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        RawValue::from_ron(&self.0)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn keeps_enum_variants_verbatim() {
        let map: BTreeMap<String, RonText> =
            ron::from_str(r#"{"accent": Blue, "blur_passes": 4}"#).expect("parses");
        assert_eq!(map["accent"].as_str(), "Blue");
        assert_eq!(map["blur_passes"].as_str(), "4");
        assert_eq!(ron::to_string(&map).expect("serializes"), r#"{"accent":Blue,"blur_passes":4}"#);
    }
}
