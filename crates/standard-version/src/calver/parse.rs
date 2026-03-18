//! Format string parsing for calver.
//!
//! Tokenises the calver format string and provides helpers to render
//! date segments from the resulting token list.

use super::{CalverDate, CalverError};

/// A parsed token from the calver format string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Token {
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
pub(super) fn parse_format(format: &str) -> Result<Vec<Token>, CalverError> {
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
pub(super) fn build_date_prefix(tokens: &[Token], date: CalverDate) -> String {
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
pub(super) fn build_date_suffix(tokens: &[Token], date: CalverDate) -> String {
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

/// Find the primary separator character from the format tokens.
pub(super) fn find_separator(tokens: &[Token]) -> String {
    for token in tokens {
        if let Token::Separator(s) = token {
            // Return first char as the separator.
            return s.chars().next().unwrap().to_string();
        }
    }
    ".".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
