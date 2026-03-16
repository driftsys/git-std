//! Calendar versioning (calver) support.
//!
//! Computes the next calver version from a format string, the current date,
//! and the previous version string. The format string uses tokens like
//! `YYYY`, `MM`, `PATCH`, etc.
//!
//! This module is pure — it takes the date as a parameter and performs no I/O.

/// Date information needed for calver computation.
///
/// All fields are simple integers — the caller is responsible for computing
/// them from the current date. This keeps the library pure (no clock access).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalverDate {
    /// Full year (e.g. 2026).
    pub year: u32,
    /// Month (1–12).
    pub month: u32,
    /// Day of month (1–31).
    pub day: u32,
    /// ISO week number (1–53).
    pub iso_week: u32,
    /// ISO day of week (1=Monday, 7=Sunday).
    pub day_of_week: u32,
}

/// The default calver format when none is specified.
pub const DEFAULT_FORMAT: &str = "YYYY.MM.PATCH";

/// Errors that can occur during calver computation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CalverError {
    /// The format string contains no `PATCH` token.
    #[error("calver format must contain the PATCH token")]
    NoPatchToken,
    /// The format string contains an unrecognised token.
    #[error("unknown calver format token: {0}")]
    UnknownToken(String),
    /// The format string is empty.
    #[error("calver format string is empty")]
    EmptyFormat,
}

/// A parsed token from the calver format string.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    /// Full year (e.g. `2026`).
    FullYear,
    /// Short year (e.g. `26`).
    ShortYear,
    /// Zero-padded month (e.g. `03`).
    ZeroPaddedMonth,
    /// Month without padding (e.g. `3`).
    Month,
    /// ISO week number (e.g. `11`).
    IsoWeek,
    /// Day of month (e.g. `13`).
    Day,
    /// Auto-incrementing patch counter.
    Patch,
    /// A literal separator (e.g. `.`).
    Separator(String),
}

/// Parse a calver format string into tokens.
fn parse_format(format: &str) -> Result<Vec<Token>, CalverError> {
    if format.is_empty() {
        return Err(CalverError::EmptyFormat);
    }

    let mut tokens = Vec::new();
    let mut remaining = format;

    while !remaining.is_empty() {
        // Try to match known tokens (longest first to avoid ambiguity).
        if let Some(rest) = remaining.strip_prefix("YYYY") {
            tokens.push(Token::FullYear);
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("YY") {
            tokens.push(Token::ShortYear);
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("0M") {
            tokens.push(Token::ZeroPaddedMonth);
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("MM") {
            tokens.push(Token::Month);
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("WW") {
            tokens.push(Token::IsoWeek);
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("DD") {
            tokens.push(Token::Day);
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("PATCH") {
            tokens.push(Token::Patch);
            remaining = rest;
        } else {
            // Consume separator characters (`.`, `-`, etc.).
            let ch = remaining.chars().next().unwrap();
            if ch == '.' || ch == '-' || ch == '_' {
                // Merge consecutive separators of the same kind.
                if let Some(Token::Separator(s)) = tokens.last_mut() {
                    s.push(ch);
                } else {
                    tokens.push(Token::Separator(ch.to_string()));
                }
                remaining = &remaining[ch.len_utf8()..];
            } else {
                // Unknown character sequence — find the next known token boundary.
                return Err(CalverError::UnknownToken(remaining.to_string()));
            }
        }
    }

    // Validate that PATCH is present.
    if !tokens.iter().any(|t| matches!(t, Token::Patch)) {
        return Err(CalverError::NoPatchToken);
    }

    Ok(tokens)
}

/// Build the date prefix from the format (everything before PATCH, including
/// the separator before PATCH).
fn build_date_prefix(tokens: &[Token], date: CalverDate) -> String {
    let mut prefix = String::new();
    for token in tokens {
        match token {
            Token::Patch => break,
            Token::FullYear => prefix.push_str(&date.year.to_string()),
            Token::ShortYear => prefix.push_str(&format!("{}", date.year % 100)),
            Token::ZeroPaddedMonth => prefix.push_str(&format!("{:02}", date.month)),
            Token::Month => prefix.push_str(&date.month.to_string()),
            Token::IsoWeek => prefix.push_str(&date.iso_week.to_string()),
            Token::Day => prefix.push_str(&date.day.to_string()),
            Token::Separator(s) => prefix.push_str(s),
        }
    }
    prefix
}

/// Build the suffix after PATCH from the format tokens.
fn build_date_suffix(tokens: &[Token], date: CalverDate) -> String {
    let mut suffix = String::new();
    let mut past_patch = false;
    for token in tokens {
        if matches!(token, Token::Patch) {
            past_patch = true;
            continue;
        }
        if !past_patch {
            continue;
        }
        match token {
            Token::FullYear => suffix.push_str(&date.year.to_string()),
            Token::ShortYear => suffix.push_str(&format!("{}", date.year % 100)),
            Token::ZeroPaddedMonth => suffix.push_str(&format!("{:02}", date.month)),
            Token::Month => suffix.push_str(&date.month.to_string()),
            Token::IsoWeek => suffix.push_str(&date.iso_week.to_string()),
            Token::Day => suffix.push_str(&date.day.to_string()),
            Token::Separator(s) => suffix.push_str(s),
            Token::Patch => {} // only one PATCH allowed, already past it
        }
    }
    suffix
}

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

/// Check if the date segments of the previous version match the current date.
fn date_segments_match(previous: &str, tokens: &[Token], date: CalverDate) -> bool {
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
fn extract_patch(previous: &str, tokens: &[Token]) -> u64 {
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

/// Find the primary separator character from the format tokens.
fn find_separator(tokens: &[Token]) -> String {
    for token in tokens {
        if let Token::Separator(s) = token {
            // Return first char as the separator.
            return s.chars().next().unwrap().to_string();
        }
    }
    ".".to_string()
}

/// Validate a calver format string without computing a version.
///
/// Returns `Ok(())` if the format is valid, or an error describing the problem.
pub fn validate_format(format: &str) -> Result<(), CalverError> {
    parse_format(format)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // ── Format parsing ──────────────────────────────────────────────

    #[test]
    fn parse_default_format() {
        let tokens = parse_format("YYYY.MM.PATCH").unwrap();
        assert_eq!(tokens.len(), 5); // YYYY, ., MM, ., PATCH
    }

    #[test]
    fn parse_zero_padded_month() {
        let tokens = parse_format("YYYY.0M.PATCH").unwrap();
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[2], Token::ZeroPaddedMonth);
    }

    #[test]
    fn parse_daily_format() {
        let tokens = parse_format("YYYY.MM.DD.PATCH").unwrap();
        assert_eq!(tokens.len(), 7); // YYYY . MM . DD . PATCH
    }

    #[test]
    fn parse_weekly_format() {
        let tokens = parse_format("YY.WW.PATCH").unwrap();
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0], Token::ShortYear);
        assert_eq!(tokens[2], Token::IsoWeek);
    }

    #[test]
    fn error_no_patch_token() {
        let err = parse_format("YYYY.MM").unwrap_err();
        assert_eq!(err, CalverError::NoPatchToken);
    }

    #[test]
    fn error_empty_format() {
        let err = parse_format("").unwrap_err();
        assert_eq!(err, CalverError::EmptyFormat);
    }

    #[test]
    fn error_unknown_token() {
        let err = parse_format("YYYY.MM.PATCH.UNKNOWN").unwrap_err();
        assert!(matches!(err, CalverError::UnknownToken(_)));
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
