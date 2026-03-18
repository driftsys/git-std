use super::load::{DEFAULT_TYPES, default_types, load, parse_config};
use super::{ProjectConfig, Scheme, ScopesConfig, discover_scopes};

#[test]
fn default_types_when_no_config() {
    let dir = tempfile::tempdir().unwrap();
    let config = load(dir.path());
    assert_eq!(config.types.len(), DEFAULT_TYPES.len());
    assert!(config.types.contains(&"feat".to_string()));
    assert_eq!(config.scopes, ScopesConfig::None);
}

#[test]
fn custom_types() {
    let config = parse_config(r#"types = ["feat", "fix", "custom"]"#);
    assert_eq!(config.types, vec!["feat", "fix", "custom"]);
}

#[test]
fn scopes_explicit_list() {
    let config = parse_config("scopes = [\"auth\", \"api\"]\n");
    assert_eq!(
        config.scopes,
        ScopesConfig::List(vec!["auth".to_string(), "api".to_string()])
    );
}

#[test]
fn scopes_auto() {
    let config = parse_config("scopes = \"auto\"\n");
    assert_eq!(config.scopes, ScopesConfig::Auto);
}

#[test]
fn no_scopes_means_none() {
    let config = parse_config(r#"types = ["feat"]"#);
    assert_eq!(config.scopes, ScopesConfig::None);
}

#[test]
fn invalid_toml_uses_defaults() {
    let config = parse_config("not valid toml {{{{");
    assert_eq!(config.types.len(), DEFAULT_TYPES.len());
}

#[test]
fn to_lint_config_not_strict() {
    let dir = tempfile::tempdir().unwrap();
    let config = ProjectConfig {
        types: vec!["feat".into()],
        scopes: ScopesConfig::List(vec!["auth".into()]),
        ..Default::default()
    };
    let lint = config.to_lint_config(false, dir.path());
    assert!(lint.types.is_none());
    assert!(lint.scopes.is_none());
    assert!(!lint.require_scope);
}

#[test]
fn to_lint_config_strict() {
    let dir = tempfile::tempdir().unwrap();
    let config = ProjectConfig {
        types: vec!["feat".into()],
        scopes: ScopesConfig::List(vec!["auth".into()]),
        ..Default::default()
    };
    let lint = config.to_lint_config(true, dir.path());
    assert_eq!(lint.types, Some(vec!["feat".into()]));
    assert_eq!(lint.scopes, Some(vec!["auth".into()]));
    assert!(lint.require_scope);
}

#[test]
fn to_lint_config_strict_no_scopes() {
    let dir = tempfile::tempdir().unwrap();
    let config = ProjectConfig {
        types: vec!["feat".into()],
        scopes: ScopesConfig::None,
        ..Default::default()
    };
    let lint = config.to_lint_config(true, dir.path());
    assert!(lint.scopes.is_none());
    assert!(!lint.require_scope);
}

#[test]
fn strict_from_config() {
    let config = parse_config("strict = true\n");
    assert!(config.strict);
}

#[test]
fn strict_default_false() {
    let config = parse_config(r#"types = ["feat"]"#);
    assert!(!config.strict);
}

#[test]
fn to_lint_config_strict_from_config() {
    let dir = tempfile::tempdir().unwrap();
    let config = ProjectConfig {
        types: vec!["feat".into()],
        scopes: ScopesConfig::List(vec!["auth".into()]),
        strict: true,
        ..Default::default()
    };
    // strict=true in config, flag=false → still strict
    let lint = config.to_lint_config(false, dir.path());
    assert_eq!(lint.types, Some(vec!["feat".into()]));
    assert_eq!(lint.scopes, Some(vec!["auth".into()]));
    assert!(lint.require_scope);
}

#[test]
fn version_files_parsed() {
    let config = parse_config(
        r#"
[[version_files]]
path = "pom.xml"
regex = '<version>([^<]+)</version>'

[[version_files]]
path = "Chart.yaml"
regex = 'version:\s*(.+)'
"#,
    );
    assert_eq!(config.version_files.len(), 2);
    assert_eq!(config.version_files[0].path, "pom.xml");
    assert_eq!(config.version_files[0].regex, "<version>([^<]+)</version>");
    assert_eq!(config.version_files[1].path, "Chart.yaml");
}

#[test]
fn version_files_default_empty() {
    let config = parse_config(r#"types = ["feat"]"#);
    assert!(config.version_files.is_empty());
}

#[test]
fn scheme_defaults_to_semver() {
    let config = parse_config(r#"types = ["feat"]"#);
    assert_eq!(config.scheme, Scheme::Semver);
}

#[test]
fn scheme_calver_parsed() {
    let config = parse_config("scheme = \"calver\"\n");
    assert_eq!(config.scheme, Scheme::Calver);
}

#[test]
fn scheme_patch_parsed() {
    let config = parse_config("scheme = \"patch\"\n");
    assert_eq!(config.scheme, Scheme::Patch);
}

#[test]
fn scheme_unknown_falls_back_to_semver() {
    let config = parse_config("scheme = \"unknown\"\n");
    assert_eq!(config.scheme, Scheme::Semver);
}

#[test]
fn discover_scopes_from_crates() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("crates/auth")).unwrap();
    std::fs::create_dir_all(dir.path().join("crates/api")).unwrap();
    let scopes = discover_scopes(dir.path());
    assert_eq!(scopes, vec!["api", "auth"]);
}

#[test]
fn discover_scopes_from_packages_and_modules() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("packages/ui")).unwrap();
    std::fs::create_dir_all(dir.path().join("modules/core")).unwrap();
    let scopes = discover_scopes(dir.path());
    assert_eq!(scopes, vec!["core", "ui"]);
}

#[test]
fn discover_scopes_deduplicates() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("crates/shared")).unwrap();
    std::fs::create_dir_all(dir.path().join("packages/shared")).unwrap();
    let scopes = discover_scopes(dir.path());
    assert_eq!(scopes, vec!["shared"]);
}

#[test]
fn discover_scopes_ignores_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("crates/auth")).unwrap();
    std::fs::write(dir.path().join("crates/README.md"), "hi").unwrap();
    let scopes = discover_scopes(dir.path());
    assert_eq!(scopes, vec!["auth"]);
}

#[test]
fn discover_scopes_empty_when_no_matching_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let scopes = discover_scopes(dir.path());
    assert!(scopes.is_empty());
}

#[test]
fn discover_scopes_non_standard_names() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("crates/my-crate_v2")).unwrap();
    std::fs::create_dir_all(dir.path().join("crates/123")).unwrap();
    let scopes = discover_scopes(dir.path());
    assert_eq!(scopes, vec!["123", "my-crate_v2"]);
}

#[test]
fn to_lint_config_auto_discovers_scopes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("crates/auth")).unwrap();
    std::fs::create_dir_all(dir.path().join("crates/api")).unwrap();
    let config = ProjectConfig {
        types: vec!["feat".into()],
        scopes: ScopesConfig::Auto,
        ..Default::default()
    };
    let lint = config.to_lint_config(true, dir.path());
    assert_eq!(lint.scopes, Some(vec!["api".into(), "auth".into()]));
    assert!(lint.require_scope);
}

#[test]
fn to_lint_config_auto_empty_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let config = ProjectConfig {
        types: vec!["feat".into()],
        scopes: ScopesConfig::Auto,
        ..Default::default()
    };
    let lint = config.to_lint_config(true, dir.path());
    assert!(lint.scopes.is_none());
    assert!(!lint.require_scope);
}

#[test]
fn resolved_scopes_auto() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("packages/web")).unwrap();
    let config = ProjectConfig {
        scopes: ScopesConfig::Auto,
        ..Default::default()
    };
    assert_eq!(config.resolved_scopes(dir.path()), vec!["web"]);
}

#[test]
fn resolved_scopes_list() {
    let dir = tempfile::tempdir().unwrap();
    let config = ProjectConfig {
        scopes: ScopesConfig::List(vec!["auth".into()]),
        ..Default::default()
    };
    assert_eq!(config.resolved_scopes(dir.path()), vec!["auth"]);
}

#[test]
fn resolved_scopes_none() {
    let dir = tempfile::tempdir().unwrap();
    let config = ProjectConfig {
        scopes: ScopesConfig::None,
        ..Default::default()
    };
    assert!(config.resolved_scopes(dir.path()).is_empty());
}

#[test]
fn calver_format_default() {
    let config = parse_config(r#"types = ["feat"]"#);
    assert_eq!(
        config.versioning.calver_format,
        standard_version::calver::DEFAULT_FORMAT
    );
}

#[test]
fn calver_format_custom() {
    let config = parse_config(
        r#"
[versioning]
calver_format = "YYYY.0M.PATCH"
"#,
    );
    assert_eq!(config.versioning.calver_format, "YYYY.0M.PATCH");
}

#[test]
fn calver_format_valid_no_fallback() {
    let config = parse_config(
        r#"
scheme = "calver"

[versioning]
calver_format = "YYYY.0M.PATCH"
"#,
    );
    assert_eq!(config.scheme, Scheme::Calver);
    assert_eq!(config.versioning.calver_format, "YYYY.0M.PATCH");
}

#[test]
fn calver_format_invalid_falls_back_to_default() {
    let config = parse_config(
        r#"
scheme = "calver"

[versioning]
calver_format = "YYYY.INVALID"
"#,
    );
    assert_eq!(config.scheme, Scheme::Calver);
    assert_eq!(
        config.versioning.calver_format,
        standard_version::calver::DEFAULT_FORMAT
    );
}

#[test]
fn calver_format_yy_0m_patch() {
    let fmt = "YY.0M.PATCH";
    assert!(standard_version::calver::validate_format(fmt).is_ok());
    let config = parse_config(&format!("[versioning]\ncalver_format = \"{fmt}\"\n"));
    assert_eq!(config.versioning.calver_format, fmt);
}

#[test]
fn calver_format_yyyy_ww_patch() {
    let fmt = "YYYY.WW.PATCH";
    assert!(standard_version::calver::validate_format(fmt).is_ok());
    let config = parse_config(&format!("[versioning]\ncalver_format = \"{fmt}\"\n"));
    assert_eq!(config.versioning.calver_format, fmt);
}

#[test]
fn calver_tokens_yy() {
    assert!(standard_version::calver::validate_format("YY.PATCH").is_ok());
}

#[test]
fn calver_tokens_0m() {
    assert!(standard_version::calver::validate_format("YYYY.0M.PATCH").is_ok());
}

#[test]
fn calver_tokens_dd() {
    assert!(standard_version::calver::validate_format("YYYY.MM.DD.PATCH").is_ok());
}

#[test]
fn calver_tokens_ww() {
    assert!(standard_version::calver::validate_format("YY.WW.PATCH").is_ok());
}

#[test]
fn version_files_with_regex_pattern() {
    let config = parse_config(
        r#"
[[version_files]]
path = "build.gradle"
regex = 'version\s*=\s*"([^"]+)"'

[[version_files]]
path = "setup.py"
regex = 'version="([^"]+)"'
"#,
    );
    assert_eq!(config.version_files.len(), 2);
    assert_eq!(config.version_files[0].path, "build.gradle");
    assert!(config.version_files[0].regex.contains("version"));
    assert_eq!(config.version_files[1].path, "setup.py");
}

#[test]
fn version_files_missing_path_skipped() {
    let config = parse_config(
        r#"
[[version_files]]
regex = 'version="([^"]+)"'
"#,
    );
    assert!(config.version_files.is_empty());
}

#[test]
fn changelog_hidden_types() {
    let config = parse_config(
        r#"
[changelog]
hidden = ["chore", "ci", "test"]
"#,
    );
    assert_eq!(
        config.changelog.hidden,
        Some(vec![
            "chore".to_string(),
            "ci".to_string(),
            "test".to_string()
        ])
    );
}

#[test]
fn changelog_hidden_default_none() {
    let config = parse_config(r#"types = ["feat"]"#);
    assert!(config.changelog.hidden.is_none());
}

#[test]
fn empty_toml_uses_defaults() {
    let config = parse_config("");
    assert_eq!(config.types.len(), DEFAULT_TYPES.len());
    assert_eq!(config.scopes, ScopesConfig::None);
    assert!(!config.strict);
    assert_eq!(config.scheme, Scheme::Semver);
    assert!(config.version_files.is_empty());
    assert!(config.changelog.hidden.is_none());
}

#[test]
fn malformed_toml_warns_and_uses_defaults() {
    let config = parse_config("{{invalid toml content!!");
    assert_eq!(config.types.len(), DEFAULT_TYPES.len());
    assert_eq!(config.scheme, Scheme::Semver);
    assert!(!config.strict);
}

#[test]
fn scheme_semver_explicit() {
    let config = parse_config("scheme = \"semver\"\n");
    assert_eq!(config.scheme, Scheme::Semver);
}

#[test]
fn non_calver_scheme_ignores_invalid_format() {
    let config = parse_config(
        r#"
scheme = "semver"

[versioning]
calver_format = "YYYY.INVALID"
"#,
    );
    assert_eq!(config.scheme, Scheme::Semver);
    // Invalid format is kept as-is because scheme is not calver.
    assert_eq!(config.versioning.calver_format, "YYYY.INVALID");
}

#[test]
fn default_types_function_returns_all_types() {
    let types = default_types();
    assert_eq!(types.len(), DEFAULT_TYPES.len());
    for t in DEFAULT_TYPES {
        assert!(types.contains(&t.to_string()));
    }
}
