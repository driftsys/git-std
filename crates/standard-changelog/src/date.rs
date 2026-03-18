//! Date utility functions for changelog generation.

/// Convert days since Unix epoch to (year, month, day).
///
/// Uses the algorithm from <http://howardhinnant.github.io/date_algorithms.html>.
pub fn days_to_date(mut days: i64) -> (i64, i64, i64) {
    days += 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Format a Unix timestamp (seconds since epoch) as `YYYY-MM-DD`.
///
/// ```
/// assert_eq!(standard_changelog::format_date(1_710_374_400), "2024-03-14");
/// ```
pub fn format_date(unix_secs: i64) -> String {
    let days = unix_secs / 86400;
    let (year, month, day) = days_to_date(days);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_to_date_epoch() {
        assert_eq!(days_to_date(0), (1970, 1, 1));
    }

    #[test]
    fn days_to_date_known_date() {
        // 2026-03-13 is day 20525
        assert_eq!(days_to_date(20525), (2026, 3, 13));
    }

    #[test]
    fn format_date_epoch() {
        assert_eq!(format_date(0), "1970-01-01");
    }

    #[test]
    fn format_date_known_timestamp() {
        // 2024-01-01 00:00:00 UTC = 1704067200
        assert_eq!(format_date(1704067200), "2024-01-01");
    }
}
