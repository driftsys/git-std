//! Bump calculation for calver.
//!
//! Computes the next version string given a format, current date, and
//! optional previous version.

use super::parse::{build_date_prefix, build_date_suffix, find_separator, parse_format};
use super::{CalverDate, CalverError};

/// Compute the next calver version string.
///
/// # Arguments
///
/// * `format` — The calver format string (e.g. `"YYYY.MM.PATCH"`).
/// * `date` — The current date.
/// * `previous_version` — The previous version string (without tag prefix), or
///   `None` if this is the first release.
///
/// # Returns
///
/// The next version string (e.g. `"2026.3.0"` or `"2026.3.1"`).
///
/// # Errors
///
/// Returns a [`CalverError`] if the format string is invalid.
pub fn next_version(
    format: &str,
    date: CalverDate,
    previous_version: Option<&str>,
) -> Result<String, CalverError> {
    let tokens = parse_format(format)?;

    let date_prefix = build_date_prefix(&tokens, date);
    let date_suffix = build_date_suffix(&tokens, date);

    // Determine patch number.
    let patch = match previous_version {
        Some(prev) => {
            // Check if the date segments match.
            if date_segments_match(prev, &tokens, date) {
                // Extract the current patch number and increment.
                extract_patch(prev, &tokens) + 1
            } else {
                0
            }
        }
        None => 0,
    };

    Ok(format!("{date_prefix}{patch}{date_suffix}"))
}

/// Validate a calver format string without computing a version.
///
/// Returns `Ok(())` if the format is valid, or an error describing the problem.
pub fn validate_format(format: &str) -> Result<(), CalverError> {
    parse_format(format)?;
    Ok(())
}

/// Check if the date segments of the previous version match the current date.
fn date_segments_match(previous: &str, tokens: &[super::parse::Token], date: CalverDate) -> bool {
    // Build the expected date prefix from the current date.
    let expected_prefix = build_date_prefix(tokens, date);
    // Build the expected date suffix from the current date.
    let expected_suffix = build_date_suffix(tokens, date);

    // The previous version should start with the date prefix and end with the date suffix.
    let prefix_matches = previous.starts_with(&expected_prefix);
    let suffix_matches = if expected_suffix.is_empty() {
        true
    } else {
        previous.ends_with(&expected_suffix)
    };

    prefix_matches && suffix_matches
}

/// Extract the patch number from a previous version string.
fn extract_patch(previous: &str, tokens: &[super::parse::Token]) -> u64 {
    use super::parse::Token;

    // Count the number of segments before PATCH and after PATCH.
    let mut segments_before_patch = 0;
    let mut segments_after_patch = 0;
    let mut past_patch = false;
    for token in tokens {
        match token {
            Token::Separator(_) => {}
            Token::Patch => {
                past_patch = true;
            }
            _ => {
                if past_patch {
                    segments_after_patch += 1;
                } else {
                    segments_before_patch += 1;
                }
            }
        }
    }

    // Split the version by the separator (detect from tokens).
    let sep = find_separator(tokens);
    let parts: Vec<&str> = previous.split(&sep).collect();

    // The patch is at index `segments_before_patch` from the left.
    if parts.len() > segments_before_patch {
        let patch_idx = segments_before_patch;
        // If there are segments after PATCH, the patch is not the last segment.
        let _ = segments_after_patch; // used for validation
        parts[patch_idx].parse().unwrap_or(0)
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calver::CalverDate;

    fn date_2026_03() -> CalverDate {
        CalverDate {
            year: 2026,
            month: 3,
            day: 16,
            iso_week: 12,
            day_of_week: 1, // Monday
        }
    }

    fn date_2026_04() -> CalverDate {
        CalverDate {
            year: 2026,
            month: 4,
            day: 1,
            iso_week: 14,
            day_of_week: 3, // Wednesday
        }
    }

    // ── First release ───────────────────────────────────────────────

    #[test]
    fn first_release_default_format() {
        let v = next_version("YYYY.MM.PATCH", date_2026_03(), None).unwrap();
        assert_eq!(v, "2026.3.0");
    }

    #[test]
    fn first_release_zero_padded() {
        let v = next_version("YYYY.0M.PATCH", date_2026_03(), None).unwrap();
        assert_eq!(v, "2026.03.0");
    }

    #[test]
    fn first_release_daily() {
        let v = next_version("YYYY.MM.DD.PATCH", date_2026_03(), None).unwrap();
        assert_eq!(v, "2026.3.16.0");
    }

    #[test]
    fn first_release_short_year() {
        let v = next_version("YY.MM.PATCH", date_2026_03(), None).unwrap();
        assert_eq!(v, "26.3.0");
    }

    #[test]
    fn first_release_weekly() {
        let v = next_version("YY.WW.PATCH", date_2026_03(), None).unwrap();
        assert_eq!(v, "26.12.0");
    }

    // ── Patch increment (same period) ───────────────────────────────

    #[test]
    fn patch_increments_same_month() {
        let v = next_version("YYYY.MM.PATCH", date_2026_03(), Some("2026.3.0")).unwrap();
        assert_eq!(v, "2026.3.1");
    }

    #[test]
    fn patch_increments_twice() {
        let v = next_version("YYYY.MM.PATCH", date_2026_03(), Some("2026.3.4")).unwrap();
        assert_eq!(v, "2026.3.5");
    }

    #[test]
    fn patch_increments_zero_padded() {
        let v = next_version("YYYY.0M.PATCH", date_2026_03(), Some("2026.03.2")).unwrap();
        assert_eq!(v, "2026.03.3");
    }

    #[test]
    fn patch_increments_daily() {
        let v = next_version("YYYY.MM.DD.PATCH", date_2026_03(), Some("2026.3.16.0")).unwrap();
        assert_eq!(v, "2026.3.16.1");
    }

    // ── Patch reset (new period) ────────────────────────────────────

    #[test]
    fn patch_resets_new_month() {
        let v = next_version("YYYY.MM.PATCH", date_2026_04(), Some("2026.3.5")).unwrap();
        assert_eq!(v, "2026.4.0");
    }

    #[test]
    fn patch_resets_new_year() {
        let date = CalverDate {
            year: 2027,
            month: 1,
            day: 1,
            iso_week: 53,
            day_of_week: 5,
        };
        let v = next_version("YYYY.MM.PATCH", date, Some("2026.12.3")).unwrap();
        assert_eq!(v, "2027.1.0");
    }

    #[test]
    fn patch_resets_new_day() {
        let date = CalverDate {
            year: 2026,
            month: 3,
            day: 17,
            iso_week: 12,
            day_of_week: 2,
        };
        let v = next_version("YYYY.MM.DD.PATCH", date, Some("2026.3.16.3")).unwrap();
        assert_eq!(v, "2026.3.17.0");
    }

    #[test]
    fn patch_resets_new_week() {
        let date = CalverDate {
            year: 2026,
            month: 3,
            day: 23,
            iso_week: 13,
            day_of_week: 1,
        };
        let v = next_version("YY.WW.PATCH", date, Some("26.12.2")).unwrap();
        assert_eq!(v, "26.13.0");
    }

    // ── Format validation ───────────────────────────────────────────

    #[test]
    fn validate_valid_format() {
        assert!(validate_format("YYYY.MM.PATCH").is_ok());
        assert!(validate_format("YYYY.0M.PATCH").is_ok());
        assert!(validate_format("YY.WW.PATCH").is_ok());
        assert!(validate_format("YYYY.MM.DD.PATCH").is_ok());
    }

    #[test]
    fn validate_invalid_format() {
        assert!(validate_format("YYYY.MM").is_err());
        assert!(validate_format("").is_err());
    }

    // ── Edge cases ──────────────────────────────────────────────────

    #[test]
    fn previous_version_is_completely_different() {
        // Previous version from a totally different format/period.
        let v = next_version("YYYY.MM.PATCH", date_2026_03(), Some("1.2.3")).unwrap();
        assert_eq!(v, "2026.3.0");
    }

    #[test]
    fn previous_version_unparseable_patch() {
        // If the patch segment isn't a number, treat as 0 and reset.
        let v = next_version("YYYY.MM.PATCH", date_2026_03(), Some("2026.3.abc")).unwrap();
        assert_eq!(v, "2026.3.1");
    }

    #[test]
    fn dash_separator() {
        let v = next_version("YYYY-MM-PATCH", date_2026_03(), None).unwrap();
        assert_eq!(v, "2026-3-0");
    }

    #[test]
    fn dash_separator_increment() {
        let v = next_version("YYYY-MM-PATCH", date_2026_03(), Some("2026-3-2")).unwrap();
        assert_eq!(v, "2026-3-3");
    }

    // ── CalverError Display ─────────────────────────────────────────

    #[test]
    fn error_display() {
        use crate::calver::CalverError;
        assert_eq!(
            CalverError::NoPatchToken.to_string(),
            "calver format must contain the PATCH token"
        );
        assert_eq!(
            CalverError::EmptyFormat.to_string(),
            "calver format string is empty"
        );
        assert!(
            CalverError::UnknownToken("X".into())
                .to_string()
                .contains("unknown calver format token")
        );
    }
}
