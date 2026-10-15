use crate::lucide::ICONS;

/// The SVG body of the Lucide icon `name` (e.g. `battery-charging`).
pub fn lucide(name: &str) -> Option<&'static str> {
    ICONS
        .binary_search_by(|(candidate, _)| candidate.cmp(&name))
        .ok()
        .map(|index| ICONS[index].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_finds_common_icons() {
        assert!(ICONS.windows(2).all(|pair| pair[0].0 < pair[1].0));
        for name in ["cpu", "battery-charging", "music", "skip-forward", "sun", "wifi"] {
            assert!(lucide(name).is_some(), "{name}");
        }
        assert!(lucide("not-an-icon").is_none());
    }
}
