//! `git std version` — lightweight, scriptable version queries.
//!
//! Queries the current version from the latest git tag, and optionally
//! computes derived information such as the next version, bump label,
//! version code, or a cargo-style describe string.

use std::path::Path;

use crate::app::OutputFormat;
use crate::config::{ProjectConfig, Scheme};
use crate::git;
use crate::ui;

/// Options for the version subcommand.
pub struct VersionOptions {
    /// Print cargo-style describe string (distance + hash + dirty flag).
    pub describe: bool,
    /// Compute and print the next version from conventional commits.
    pub next: bool,
    /// Print the bump label (major/minor/patch).
    pub label: bool,
    /// Print the version code integer.
    pub code: bool,
    /// Output format.
    pub format: OutputFormat,
}

/// Run the version subcommand. Returns the process exit code.
pub fn run(config: &ProjectConfig, opts: &VersionOptions) -> i32 {
    let dir = Path::new(".");

    match config.scheme {
        Scheme::Calver => run_calver(config, opts, dir),
        _ => run_semver(config, opts, dir),
    }
}

// ---------------------------------------------------------------------------
// Semver path
// ---------------------------------------------------------------------------

fn run_semver(config: &ProjectConfig, opts: &VersionOptions, dir: &Path) -> i32 {
    let tag_prefix = &config.versioning.tag_prefix;

    let current = match git::find_latest_version_tag(dir, tag_prefix) {
        Ok(Some((oid, ver))) => (oid, ver),
        Ok(None) => {
            ui::error("no version tag found");
            return 1;
        }
        Err(e) => {
            ui::error(&e.to_string());
            return 1;
        }
    };

    let (tag_oid, cur_ver) = &current;

    let version_str = cur_ver.to_string();
    let describe_str = if opts.describe {
        match build_describe(dir, tag_oid, &version_str) {
            Ok(s) => Some(s),
            Err(e) => {
                ui::error(&e);
                return 1;
            }
        }
    } else {
        None
    };

    let (next_str, bump_label) = if opts.next || opts.label || opts.format == OutputFormat::Json {
        match compute_next_semver(dir, cur_ver, tag_oid) {
            Ok((n, l)) => (Some(n), Some(l)),
            Err(e) => {
                ui::error(&e);
                return 1;
            }
        }
    } else {
        (None, None)
    };

    let code_val = if opts.code || opts.format == OutputFormat::Json {
        Some(semver_code(cur_ver))
    } else {
        None
    };

    match opts.format {
        OutputFormat::Json => {
            print_json(
                &version_str,
                describe_str.as_deref(),
                next_str.as_deref(),
                bump_label.as_deref(),
                code_val,
            );
            0
        }
        OutputFormat::Text => print_text(
            opts,
            &version_str,
            describe_str.as_deref(),
            next_str.as_deref(),
            bump_label.as_deref(),
            code_val,
        ),
    }
}

// ---------------------------------------------------------------------------
// Calver path
// ---------------------------------------------------------------------------

fn run_calver(config: &ProjectConfig, opts: &VersionOptions, dir: &Path) -> i32 {
    let tag_prefix = &config.versioning.tag_prefix;

    let current = match git::find_latest_calver_tag(dir, tag_prefix) {
        Ok(Some((oid, ver))) => (oid, ver),
        Ok(None) => {
            ui::error("no version tag found");
            return 1;
        }
        Err(e) => {
            ui::error(&e.to_string());
            return 1;
        }
    };

    let (tag_oid, cur_ver) = &current;

    let describe_str = if opts.describe {
        match build_describe(dir, tag_oid, cur_ver) {
            Ok(s) => Some(s),
            Err(e) => {
                ui::error(&e);
                return 1;
            }
        }
    } else {
        None
    };

    let next_str = if opts.next || opts.format == OutputFormat::Json {
        match compute_next_calver(config, cur_ver) {
            Ok(n) => Some(n),
            Err(e) => {
                ui::error(&e);
                return 1;
            }
        }
    } else {
        None
    };

    let code_val = if opts.code || opts.format == OutputFormat::Json {
        match calver_code(cur_ver) {
            Ok(c) => Some(c),
            Err(e) => {
                ui::error(&e);
                return 1;
            }
        }
    } else {
        None
    };

    // For calver, --label is not meaningful (no bump level concept).
    let bump_label: Option<String> = if opts.label || opts.format == OutputFormat::Json {
        Some("calver".to_string())
    } else {
        None
    };

    match opts.format {
        OutputFormat::Json => {
            print_json(
                cur_ver,
                describe_str.as_deref(),
                next_str.as_deref(),
                bump_label.as_deref(),
                code_val,
            );
            0
        }
        OutputFormat::Text => print_text(
            opts,
            cur_ver,
            describe_str.as_deref(),
            next_str.as_deref(),
            bump_label.as_deref(),
            code_val,
        ),
    }
}

// ---------------------------------------------------------------------------
// Helpers — describe
// ---------------------------------------------------------------------------

/// Build a cargo-style describe string: `<version>[-dev.<N>][+<hash>[.dirty]]`.
///
/// - `-dev.N` is appended when HEAD is N commits ahead of the tag.
/// - `+<hash>` is appended when HEAD is ahead of the tag.
/// - `.dirty` is appended when the working tree has uncommitted changes.
fn build_describe(dir: &Path, tag_oid: &str, version: &str) -> Result<String, String> {
    let head_oid = git::head_oid(dir).map_err(|e| e.to_string())?;

    // Count commits between tag and HEAD.
    let commits = git::walk_commits(dir, &head_oid, Some(tag_oid)).map_err(|e| e.to_string())?;
    let distance = commits.len();

    let is_dirty = git::is_working_tree_dirty(dir).map_err(|e| e.to_string())?;

    // Short hash — use first 7 hex digits of HEAD.
    let short_hash = if head_oid.len() >= 7 {
        &head_oid[..7]
    } else {
        &head_oid
    };

    let mut result = version.to_string();

    if distance > 0 {
        result.push_str(&format!("-dev.{distance}"));
    }

    if distance > 0 || is_dirty {
        result.push('+');
        result.push_str(&format!("g{short_hash}"));
        if is_dirty {
            result.push_str(".dirty");
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Helpers — next version / label
// ---------------------------------------------------------------------------

fn compute_next_semver(
    dir: &Path,
    cur_ver: &semver::Version,
    tag_oid: &str,
) -> Result<(String, String), String> {
    let head_oid = git::head_oid(dir).map_err(|e| e.to_string())?;
    let raw_commits =
        git::walk_commits(dir, &head_oid, Some(tag_oid)).map_err(|e| e.to_string())?;

    let parsed: Vec<standard_commit::ConventionalCommit> = raw_commits
        .iter()
        .filter_map(|(_, msg)| standard_commit::parse(msg).ok())
        .collect();

    let bump_level = standard_version::determine_bump(&parsed);

    let (next_ver, label) = match bump_level {
        None => (cur_ver.clone(), "none".to_string()),
        Some(level) => {
            let next = standard_version::apply_bump(cur_ver, level);
            let is_pre1 = cur_ver.major == 0;
            let label_str = match (level, is_pre1) {
                (standard_version::BumpLevel::Major, true) => "minor",
                (standard_version::BumpLevel::Minor, true) => "patch",
                (standard_version::BumpLevel::Patch, _) => "patch",
                (standard_version::BumpLevel::Major, false) => "major",
                (standard_version::BumpLevel::Minor, false) => "minor",
            };
            (next, label_str.to_string())
        }
    };

    Ok((next_ver.to_string(), label))
}

fn compute_next_calver(config: &ProjectConfig, cur_ver: &str) -> Result<String, String> {
    let date = crate::cli::bump::detect::today_calver_date();
    standard_version::calver::next_version(&config.versioning.calver_format, date, Some(cur_ver))
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Helpers — version code
// ---------------------------------------------------------------------------

/// Compute version code for semver: `((MAJOR * 1_000 + MINOR) * 100 + PATCH) * 100 + stage`.
pub fn semver_code(ver: &semver::Version) -> u64 {
    let base = ((ver.major * 1_000 + ver.minor) * 100 + ver.patch) * 100;
    let stage = prerelease_stage(ver.pre.as_str());
    base + stage
}

/// Compute version code for calver: `days_since_epoch * 10_000 + MICRO * 100 + stage`.
///
/// Parses `YYYY.MM.PATCH` (and variants) — extracts the last numeric segment as MICRO.
pub fn calver_code(ver: &str) -> Result<u64, String> {
    // Split on common separators.
    let parts: Vec<&str> = ver.split(['.', '-', '_']).collect();
    if parts.len() < 3 {
        return Err(format!("cannot parse calver version: '{ver}'"));
    }

    // Parse the date segments (first one or two segments) as year and month.
    // The last segment is MICRO (PATCH in calver terms).
    let micro: u64 = parts[parts.len() - 1]
        .parse()
        .map_err(|_| format!("cannot parse MICRO in calver version: '{ver}'"))?;

    // Parse year and month to compute days since epoch.
    let year: i32 = parts[0]
        .parse()
        .map_err(|_| format!("cannot parse year in calver version: '{ver}'"))?;

    // If the year is 2-digit, adjust.
    let full_year = if year < 100 { year + 2000 } else { year };

    let month: u32 = parts[1]
        .parse()
        .map_err(|_| format!("cannot parse month in calver version: '{ver}'"))?;

    // Compute days since Unix epoch for the first day of the given month/year.
    let days = days_since_epoch(full_year, month, 1) as u64;

    Ok(days * 10_000 + micro * 100 + 99) // stable calver releases use stage 99
}

/// Compute days since Unix epoch (1970-01-01) for a given date.
///
/// Uses the Howard Hinnant civil_to_days algorithm (inverse of civil_from_days).
fn days_since_epoch(year: i32, month: u32, day: u32) -> i32 {
    // Shift year to March-based for easier leap-year math.
    let (y, m) = if month <= 2 {
        (year - 1, month + 9)
    } else {
        (year, month - 3)
    };

    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400) as u32;
    let doy = (153 * m + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i32 - 719468
}

/// Map a pre-release string to a stage integer.
///
/// | Pre-release     | Stage |
/// |-----------------|-------|
/// | (unknown)       | 1     |
/// | dev             | 9     |
/// | dev.0 – dev.28  | 10–38 |
/// | alpha           | 39    |
/// | alpha.0–alpha.18| 40–58 |
/// | beta            | 59    |
/// | beta.0–beta.18  | 60–78 |
/// | rc              | 79    |
/// | rc.0–rc.18      | 80–98 |
/// | (stable)        | 99    |
pub fn prerelease_stage(pre: &str) -> u64 {
    if pre.is_empty() {
        return 99; // stable
    }

    // Try "tag.N" pattern.
    let (tag, number) = if let Some(dot) = pre.rfind('.') {
        let tag_part = &pre[..dot];
        let num_part = &pre[dot + 1..];
        if let Ok(n) = num_part.parse::<u64>() {
            (tag_part, Some(n))
        } else {
            (pre, None)
        }
    } else {
        (pre, None)
    };

    match (tag, number) {
        ("dev", None) => 9,
        ("dev", Some(n)) if n <= 28 => 10 + n,
        ("alpha", None) => 39,
        ("alpha", Some(n)) if n <= 18 => 40 + n,
        ("beta", None) => 59,
        ("beta", Some(n)) if n <= 18 => 60 + n,
        ("rc", None) => 79,
        ("rc", Some(n)) if n <= 18 => 80 + n,
        _ => 1, // unknown
    }
}

// ---------------------------------------------------------------------------
// Output rendering
// ---------------------------------------------------------------------------

fn print_text(
    opts: &VersionOptions,
    version: &str,
    describe: Option<&str>,
    next: Option<&str>,
    label: Option<&str>,
    code: Option<u64>,
) -> i32 {
    // When multiple flags are set, print all requested fields.
    // When no flag is set (bare call), print just the version.
    let any_flag = opts.describe || opts.next || opts.label || opts.code;

    if !any_flag {
        println!("{version}");
        return 0;
    }

    if opts.describe
        && let Some(d) = describe
    {
        println!("{d}");
    }
    if opts.next
        && let Some(n) = next
    {
        println!("{n}");
    }
    if opts.label
        && let Some(l) = label
    {
        println!("{l}");
    }
    if opts.code
        && let Some(c) = code
    {
        println!("{c}");
    }

    0
}

fn print_json(
    version: &str,
    describe: Option<&str>,
    next: Option<&str>,
    label: Option<&str>,
    code: Option<u64>,
) {
    let obj = serde_json::json!({
        "version": version,
        "describe": describe,
        "next": next,
        "label": label,
        "code": code,
    });
    println!("{obj}");
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Stage table ─────────────────────────────────────────────────────────

    #[test]
    fn stage_stable() {
        assert_eq!(prerelease_stage(""), 99);
    }

    #[test]
    fn stage_dev() {
        assert_eq!(prerelease_stage("dev"), 9);
    }

    #[test]
    fn stage_dev_n() {
        assert_eq!(prerelease_stage("dev.0"), 10);
        assert_eq!(prerelease_stage("dev.28"), 38);
    }

    #[test]
    fn stage_alpha() {
        assert_eq!(prerelease_stage("alpha"), 39);
    }

    #[test]
    fn stage_alpha_n() {
        assert_eq!(prerelease_stage("alpha.0"), 40);
        assert_eq!(prerelease_stage("alpha.18"), 58);
    }

    #[test]
    fn stage_beta() {
        assert_eq!(prerelease_stage("beta"), 59);
    }

    #[test]
    fn stage_beta_n() {
        assert_eq!(prerelease_stage("beta.0"), 60);
        assert_eq!(prerelease_stage("beta.18"), 78);
    }

    #[test]
    fn stage_rc() {
        assert_eq!(prerelease_stage("rc"), 79);
    }

    #[test]
    fn stage_rc_n() {
        assert_eq!(prerelease_stage("rc.0"), 80);
        assert_eq!(prerelease_stage("rc.18"), 98);
    }

    #[test]
    fn stage_unknown() {
        assert_eq!(prerelease_stage("nightly"), 1);
    }

    // ── Semver code ─────────────────────────────────────────────────────────

    #[test]
    fn semver_code_stable() {
        // 1.2.3 stable: ((1*1000+2)*100+3)*100+99
        // = (1002*100+3)*100+99
        // = 100203*100+99
        // = 10020300+99
        // = 10020399
        let ver = semver::Version::new(1, 2, 3);
        assert_eq!(semver_code(&ver), 10_020_399);
    }

    #[test]
    fn semver_code_pre_rc() {
        // 1.0.0-rc.1: ((1*1000+0)*100+0)*100+81
        // = (1000*100+0)*100+81
        // = 100000*100+81
        // = 10000081
        let ver = semver::Version::parse("1.0.0-rc.1").unwrap();
        assert_eq!(semver_code(&ver), 10_000_081);
    }

    #[test]
    fn semver_code_zero_zero_zero_stable() {
        // 0.0.0 stable: ((0*1000+0)*100+0)*100+99 = 99
        let ver = semver::Version::new(0, 0, 0);
        assert_eq!(semver_code(&ver), 99);
    }

    #[test]
    fn semver_code_pre1_current() {
        // 0.10.2 stable: ((0*1000+10)*100+2)*100+99
        // = (10*100+2)*100+99
        // = 1002*100+99
        // = 100200+99
        // = 100299
        let ver = semver::Version::new(0, 10, 2);
        assert_eq!(semver_code(&ver), 100_299);
    }

    // ── Round-trip ──────────────────────────────────────────────────────────

    #[test]
    fn semver_code_ordering() {
        let v100 = semver::Version::new(1, 0, 0);
        let v101 = semver::Version::new(1, 0, 1);
        let v110 = semver::Version::new(1, 1, 0);
        let v200 = semver::Version::new(2, 0, 0);
        assert!(semver_code(&v100) < semver_code(&v101));
        assert!(semver_code(&v101) < semver_code(&v110));
        assert!(semver_code(&v110) < semver_code(&v200));
    }

    #[test]
    fn semver_code_prerelease_less_than_stable() {
        let pre = semver::Version::parse("1.0.0-rc.18").unwrap();
        let stable = semver::Version::new(1, 0, 0);
        assert!(semver_code(&pre) < semver_code(&stable));
    }

    // ── Days since epoch ────────────────────────────────────────────────────

    #[test]
    fn days_since_epoch_unix_epoch() {
        assert_eq!(days_since_epoch(1970, 1, 1), 0);
    }

    #[test]
    fn days_since_epoch_known_date() {
        // 2026-03-16 = day 20528 (from calver detect tests).
        assert_eq!(days_since_epoch(2026, 3, 16), 20_528);
    }

    // ── Calver code ─────────────────────────────────────────────────────────

    #[test]
    fn calver_code_basic() {
        // "2026.3.0" — March 1 2026: days * 10000 + 99 (stable, MICRO=0)
        let days = days_since_epoch(2026, 3, 1);
        let expected = days as u64 * 10_000 + 99;
        let code = calver_code("2026.3.0").unwrap();
        assert_eq!(code, expected);
    }

    #[test]
    fn calver_code_patch_1() {
        let days = days_since_epoch(2026, 3, 1);
        let expected = days as u64 * 10_000 + 100 + 99;
        let code = calver_code("2026.3.1").unwrap();
        assert_eq!(code, expected);
    }

    #[test]
    fn calver_code_ordering() {
        // Later months have higher codes.
        let code_march = calver_code("2026.3.0").unwrap();
        let code_april = calver_code("2026.4.0").unwrap();
        assert!(code_march < code_april);
    }

    #[test]
    fn calver_code_invalid() {
        assert!(calver_code("notaversion").is_err());
    }
}
