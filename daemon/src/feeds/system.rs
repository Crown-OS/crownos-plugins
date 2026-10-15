//! CPU and memory from `/proc`; CPU load is the change since the last sample.

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SystemInfo {
    /// Busy fraction of all cores since the previous sample.
    pub cpu: f64,
    pub memory: f64,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Sampler {
    previous: Option<(u64, u64)>,
}

fn cpu_counters(stat: &str) -> Option<(u64, u64)> {
    let fields: Vec<u64> = stat
        .lines()
        .next()?
        .strip_prefix("cpu ")?
        .split_whitespace()
        .filter_map(|field| field.parse().ok())
        .collect();
    let idle = fields.get(3)? + fields.get(4).copied().unwrap_or(0);
    Some((fields.iter().sum(), idle))
}

fn meminfo_kb(meminfo: &str, key: &str) -> Option<u64> {
    meminfo
        .lines()
        .find_map(|line| line.strip_prefix(key)?.trim().strip_suffix("kB")?.trim().parse().ok())
}

impl Sampler {
    pub fn sample(&mut self) -> SystemInfo {
        let stat = std::fs::read_to_string("/proc/stat").unwrap_or_default();
        let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let uptime = std::fs::read_to_string("/proc/uptime").unwrap_or_default();
        self.compute(&stat, &meminfo, &uptime)
    }

    fn compute(&mut self, stat: &str, meminfo: &str, uptime: &str) -> SystemInfo {
        let cpu = match (cpu_counters(stat), self.previous) {
            (Some((total, idle)), Some((last_total, last_idle))) if total > last_total => {
                1.0 - (idle.saturating_sub(last_idle)) as f64 / (total - last_total) as f64
            }
            _ => 0.0,
        };
        self.previous = cpu_counters(stat).or(self.previous);
        let total = meminfo_kb(meminfo, "MemTotal:").unwrap_or(0);
        let available = meminfo_kb(meminfo, "MemAvailable:").unwrap_or(total);
        let used = total.saturating_sub(available);
        SystemInfo {
            cpu: cpu.clamp(0.0, 1.0),
            memory: if total == 0 { 0.0 } else { used as f64 / total as f64 },
            memory_used_mb: used / 1024,
            memory_total_mb: total / 1024,
            uptime_secs: uptime
                .split_whitespace()
                .next()
                .and_then(|secs| secs.parse::<f64>().ok())
                .map_or(0, |secs| secs as u64),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMINFO: &str = "MemTotal:       16000000 kB\nMemAvailable:    4000000 kB\n";

    #[test]
    fn cpu_is_the_busy_share_between_samples() {
        let mut sampler = Sampler::default();
        sampler.compute("cpu  100 0 100 800 0 0 0\n", MEMINFO, "12.5 3.0");
        let info = sampler.compute("cpu  150 0 150 900 0 0 0\n", MEMINFO, "13.5 3.0");
        assert!((info.cpu - 0.5).abs() < 1e-9);
        assert!((info.memory - 0.75).abs() < 1e-9);
        assert_eq!(info.uptime_secs, 13);
    }
}
