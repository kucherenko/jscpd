//! The clock string that reports carry as their detection date.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The current UTC time as `2026-11-18T09:30:00.123Z`, the shape of
/// JavaScript's `Date.prototype.toISOString()`.
pub fn now_iso8601() -> String {
    iso8601_utc(SystemTime::now())
}

/// Format a system time in UTC with millisecond precision. It never panics:
/// a clock before 1970 gives the epoch, and a time that jiff can't hold
/// (past the year 9999) gives the seconds as a plain number.
fn iso8601_utc(time: SystemTime) -> String {
    let since_epoch = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    timestamp(since_epoch)
        .map(|timestamp| format!("{timestamp:.3}"))
        .unwrap_or_else(|| since_epoch.as_secs().to_string())
}

fn timestamp(since_epoch: Duration) -> Option<jiff::Timestamp> {
    let seconds = i64::try_from(since_epoch.as_secs()).ok()?;
    let nanoseconds = i32::try_from(since_epoch.subsec_nanos()).ok()?;
    jiff::Timestamp::new(seconds, nanoseconds).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64, nanoseconds: u32) -> SystemTime {
        UNIX_EPOCH + Duration::new(seconds, nanoseconds)
    }

    #[test]
    fn formats_utc_with_milliseconds() {
        let cases = [
            (0, 0, "1970-01-01T00:00:00.000Z"),
            (86_399, 999_999_999, "1970-01-01T23:59:59.999Z"),
            (1_709_164_800, 7_000_000, "2024-02-29T00:00:00.007Z"),
            (1_795_080_645, 123_000_000, "2026-11-19T09:30:45.123Z"),
        ];
        for (seconds, nanoseconds, expected) in cases {
            assert_eq!(
                iso8601_utc(at(seconds, nanoseconds)),
                expected,
                "seconds = {seconds}"
            );
        }
    }

    #[test]
    fn a_clock_before_1970_gives_the_epoch() {
        let before = UNIX_EPOCH - Duration::from_secs(86_400);
        assert_eq!(iso8601_utc(before), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn a_time_past_the_year_9999_gives_plain_seconds() {
        let seconds = 400_000_000_000;
        assert_eq!(iso8601_utc(at(seconds, 0)), seconds.to_string());
    }

    #[test]
    fn now_has_the_shape_of_to_iso_string() {
        let now = now_iso8601();
        let shape: String = now
            .chars()
            .map(|c| if c.is_ascii_digit() { '0' } else { c })
            .collect();
        assert_eq!(shape, "0000-00-00T00:00:00.000Z", "{now}");
    }
}
