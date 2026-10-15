//! Battery state from `/sys/class/power_supply`.

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PowerInfo {
    pub present: bool,
    pub percent: f64,
    pub charging: bool,
}

pub fn read() -> PowerInfo {
    let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") else {
        return PowerInfo::default();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            std::fs::read_to_string(path.join("type")).is_ok_and(|kind| kind.trim() == "Battery")
        })
        .map_or_else(PowerInfo::default, |battery| {
            let read = |name: &str| std::fs::read_to_string(battery.join(name)).unwrap_or_default();
            PowerInfo {
                present: true,
                percent: read("capacity").trim().parse().unwrap_or(0.0),
                charging: matches!(read("status").trim(), "Charging" | "Full"),
            }
        })
}
