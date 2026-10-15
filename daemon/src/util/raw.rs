//! Lossless edits of RON lists and string-keyed maps: items stay as their
//! own source text, so struct and enum spellings survive untouched, and are
//! only parsed into `ron::Value` to compare them.

use std::collections::BTreeMap;

use ron::value::RawValue;

pub type RawList = Vec<Box<RawValue>>;
pub type RawMap = BTreeMap<String, Box<RawValue>>;

fn options() -> ron::Options {
    ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
}

pub fn parse_list(text: &str) -> Result<RawList, ron::error::SpannedError> {
    options().from_str(text)
}

pub fn parse_map(text: &str) -> Result<RawMap, ron::error::SpannedError> {
    options().from_str(text)
}

pub fn raw(text: &str) -> Result<Box<RawValue>, ron::error::SpannedError> {
    RawValue::from_boxed_ron(text.trim().into())
}

pub fn list_text(items: &[Box<RawValue>]) -> String {
    let body: Vec<&str> = items.iter().map(|item| item.get_ron()).collect();
    format!("[{}]", body.join(","))
}

pub fn map_text(map: &RawMap) -> String {
    let body: Vec<String> = map
        .iter()
        .map(|(key, value)| format!("{key:?}:{}", value.get_ron()))
        .collect();
    format!("{{{}}}", body.join(","))
}

/// Structural equality, independent of whitespace and optional `Some(..)`.
pub fn same(a: &str, b: &str) -> bool {
    if a.trim() == b.trim() {
        return true;
    }
    match (
        options().from_str::<ron::Value>(a),
        options().from_str::<ron::Value>(b),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

pub fn position(items: &[Box<RawValue>], item: &str) -> Option<usize> {
    items.iter().position(|candidate| same(candidate.get_ron(), item))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_keep_their_item_spelling() {
        let mut items = parse_list(r#"[(app_id: Some("a"), floating: Some(true)), (title: "b")]"#)
            .expect("parses");
        items.remove(1);
        assert_eq!(list_text(&items), r#"[(app_id: Some("a"), floating: Some(true))]"#);
    }

    #[test]
    fn equality_ignores_formatting() {
        assert!(same(r#"( app_id: Some("a") )"#, r#"(app_id:Some("a"))"#));
        assert!(!same(r#""clock""#, r#""wifi""#));
    }

    #[test]
    fn maps_round_trip() {
        let mut map = parse_map(r##"{"surface.bg": "#000000"}"##).expect("parses");
        map.insert("text.body".into(), raw(r##""#ffffff""##).expect("raw"));
        let text = map_text(&map);
        let back: BTreeMap<String, String> = ron::from_str(&text).expect("typed");
        assert_eq!(back["text.body"], "#ffffff");
    }
}
