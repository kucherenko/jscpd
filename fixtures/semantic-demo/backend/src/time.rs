//! Human-readable timestamps for notification e-mails.

use chrono::{DateTime, Utc};

/// `"just now"`, `"5 minutes ago"`, `"yesterday"`, `"3 weeks ago"`, ...
pub fn relative_time(then: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let seconds = (now - then).num_seconds().max(0);
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    match () {
        _ if seconds < 45 => "just now".to_string(),
        _ if minutes < 60 => plural(minutes.max(1), "minute"),
        _ if hours < 24 => plural(hours, "hour"),
        _ if days == 1 => "yesterday".to_string(),
        _ if days < 7 => plural(days, "day"),
        _ if days < 30 => plural(days / 7, "week"),
        _ if days < 365 => plural(days / 30, "month"),
        _ => plural(days / 365, "year"),
    }
}

fn plural(count: i64, unit: &str) -> String {
    if count == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{count} {unit}s ago")
    }
}
