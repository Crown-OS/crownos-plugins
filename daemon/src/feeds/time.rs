use chrono::{Datelike, Local, Timelike};

#[derive(Debug, Clone, PartialEq)]
pub struct TimeInfo {
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub year: i32,
    pub month: u32,
    pub day: u32,
    /// 1 = Monday.
    pub weekday: u32,
    pub unix: i64,
}

pub fn now() -> TimeInfo {
    let now = Local::now();
    TimeInfo {
        hour: now.hour(),
        minute: now.minute(),
        second: now.second(),
        year: now.year(),
        month: now.month(),
        day: now.day(),
        weekday: now.weekday().number_from_monday(),
        unix: now.timestamp(),
    }
}

/// strftime-style formatting; an invalid pattern yields an empty string.
pub fn format(pattern: &str) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    match write!(out, "{}", Local::now().format(pattern)) {
        Ok(()) => out,
        Err(_) => String::new(),
    }
}
