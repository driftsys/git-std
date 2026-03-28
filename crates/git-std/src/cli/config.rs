use std::path::Path;

use crate::app::OutputFormat;
use crate::config::{self, ScopesConfig};
use crate::ui;

/// Source annotation for a configuration value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// Value came from `.git-std.toml`.
    File,
    /// Value is the built-in default.
    Default,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Source::File => ".git-std.toml",
            Source::Default => "(default)",
        }
    }
}

/// Column width for value alignment in text output.
const VALUE_COL: usize = 50;

/// Run the `config list` subcommand. Returns the process exit code.
pub fn list(dir: &Path, format: OutputFormat) -> i32 {
    let (cfg, raw) = config::load_with_raw(dir);

    let has_file = raw.is_some();
    let raw = raw.unwrap_or_default();

    let has_key = |key: &str| has_file && raw.contains_key(key);
    let has_versioning_key = |key: &str| {
        has_file
            && raw
                .get("versioning")
                .and_then(|v| v.as_table())
                .is_some_and(|t| t.contains_key(key))
    };
    let has_changelog_key = |key: &str| {
        has_file
            && raw
                .get("changelog")
                .and_then(|v| v.as_table())
                .is_some_and(|t| t.contains_key(key))
    };

    if format == OutputFormat::Json {
        return list_json(&cfg);
    }

    // ── Top-level ───────────────────────────────────────────────
    let scheme_src = if has_key("scheme") {
        Source::File
    } else {
        Source::Default
    };
    let scheme_label = match cfg.scheme {
        config::Scheme::Semver => "semver",
        config::Scheme::Calver => "calver",
        config::Scheme::Patch => "patch",
    };
    print_kv("scheme", scheme_label, scheme_src);

    let strict_src = if has_key("strict") {
        Source::File
    } else {
        Source::Default
    };
    print_kv("strict", &cfg.strict.to_string(), strict_src);

    let types_src = if has_key("types") {
        Source::File
    } else {
        Source::Default
    };
    let types_value = format_str_list(&cfg.types);
    print_kv("types", &types_value, types_src);

    let scopes_src = if has_key("scopes") {
        Source::File
    } else {
        Source::Default
    };
    match &cfg.scopes {
        ScopesConfig::None => print_kv("scopes", "none", scopes_src),
        ScopesConfig::Auto => {
            print_kv("scopes", "auto", scopes_src);
            let resolved = cfg.resolved_scopes(dir);
            if !resolved.is_empty() {
                ui::detail(&format!("resolved: {}", resolved.join(", ")));
            }
        }
        ScopesConfig::List(list) => {
            print_kv("scopes", &format_str_list(list), scopes_src);
        }
    }

    // ── [versioning] ────────────────────────────────────────────
    ui::blank();
    ui::info("[versioning]");

    let tag_prefix_src = if has_versioning_key("tag_prefix") {
        Source::File
    } else {
        Source::Default
    };
    print_kv(
        "tag_prefix",
        &format!("{:?}", cfg.versioning.tag_prefix),
        tag_prefix_src,
    );

    let prerelease_src = if has_versioning_key("prerelease_tag") {
        Source::File
    } else {
        Source::Default
    };
    print_kv(
        "prerelease_tag",
        &format!("{:?}", cfg.versioning.prerelease_tag),
        prerelease_src,
    );

    let calver_src = if has_versioning_key("calver_format") {
        Source::File
    } else {
        Source::Default
    };
    print_kv(
        "calver_format",
        &format!("{:?}", cfg.versioning.calver_format),
        calver_src,
    );

    // ── [changelog] ─────────────────────────────────────────────
    ui::blank();
    ui::info("[changelog]");

    let default_cl = standard_changelog::ChangelogConfig::default();

    let title_src = if has_changelog_key("title") {
        Source::File
    } else {
        Source::Default
    };
    let title_value = cfg.changelog.title.as_deref().unwrap_or(&default_cl.title);
    print_kv("title", &format!("{title_value:?}"), title_src);

    let hidden_src = if has_changelog_key("hidden") {
        Source::File
    } else {
        Source::Default
    };
    let hidden_value = cfg.changelog.hidden.as_ref().unwrap_or(&default_cl.hidden);
    print_kv("hidden", &format_str_list(hidden_value), hidden_src);

    let sections_src = if has_changelog_key("sections") {
        Source::File
    } else {
        Source::Default
    };
    let sections_value = cfg
        .changelog
        .sections
        .as_ref()
        .unwrap_or(&default_cl.sections);
    print_kv("sections", &format_sections(sections_value), sections_src);

    let bug_url_src = if has_changelog_key("bug_url") {
        Source::File
    } else {
        Source::Default
    };
    let bug_url_value = cfg
        .changelog
        .bug_url
        .as_deref()
        .map_or("null".to_string(), |u| format!("{u:?}"));
    print_kv("bug_url", &bug_url_value, bug_url_src);

    // ── [[version_files]] ───────────────────────────────────────
    if !cfg.version_files.is_empty() {
        ui::blank();
        ui::info("[[version_files]]");
        for vf in &cfg.version_files {
            ui::detail(&format!("path = {:?}, regex = {:?}", vf.path, vf.regex));
        }
    }

    0
}

/// Run the `config get` subcommand. Returns the process exit code.
pub fn get(dir: &Path, key: &str, format: OutputFormat) -> i32 {
    let (cfg, _raw) = config::load_with_raw(dir);
    let default_cl = standard_changelog::ChangelogConfig::default();

    match key {
        "scheme" => {
            let v = match cfg.scheme {
                config::Scheme::Semver => "semver",
                config::Scheme::Calver => "calver",
                config::Scheme::Patch => "patch",
            };
            print_value(v, format);
            0
        }
        "strict" => {
            print_value(&cfg.strict.to_string(), format);
            0
        }
        "types" => {
            if format == OutputFormat::Json {
                println!("{}", serde_json::to_string(&cfg.types).unwrap());
            } else {
                eprintln!("{}", format_str_list(&cfg.types));
            }
            0
        }
        "scopes" => {
            if format == OutputFormat::Json {
                println!("{}", serde_json::to_string(&cfg.scopes).unwrap());
            } else {
                let v = match &cfg.scopes {
                    ScopesConfig::None => "none".to_string(),
                    ScopesConfig::Auto => "auto".to_string(),
                    ScopesConfig::List(list) => format_str_list(list),
                };
                eprintln!("{v}");
            }
            0
        }
        "versioning.tag_prefix" => {
            print_value(&cfg.versioning.tag_prefix, format);
            0
        }
        "versioning.prerelease_tag" => {
            print_value(&cfg.versioning.prerelease_tag, format);
            0
        }
        "versioning.calver_format" => {
            print_value(&cfg.versioning.calver_format, format);
            0
        }
        "changelog.title" => {
            let v = cfg.changelog.title.as_deref().unwrap_or(&default_cl.title);
            print_value(v, format);
            0
        }
        "changelog.hidden" => {
            let v = cfg.changelog.hidden.as_ref().unwrap_or(&default_cl.hidden);
            if format == OutputFormat::Json {
                println!("{}", serde_json::to_string(v).unwrap());
            } else {
                eprintln!("{}", format_str_list(v));
            }
            0
        }
        "changelog.sections" => {
            let v = cfg
                .changelog
                .sections
                .as_ref()
                .unwrap_or(&default_cl.sections);
            if format == OutputFormat::Json {
                // Serialize as an object {type: section_title, ...}
                let map: serde_json::Map<String, serde_json::Value> = v
                    .iter()
                    .map(|(k, s)| (k.clone(), serde_json::Value::String(s.clone())))
                    .collect();
                println!("{}", serde_json::to_string(&map).unwrap());
            } else {
                eprintln!("{}", format_sections(v));
            }
            0
        }
        "changelog.bug_url" => {
            print_optional_value(cfg.changelog.bug_url.as_deref(), format);
            0
        }
        unknown => {
            ui::error(&format!("unknown config key: {unknown:?}"));
            ui::info("supported keys: scheme, strict, types, scopes, versioning.tag_prefix,");
            ui::info("  versioning.prerelease_tag, versioning.calver_format, changelog.title,");
            ui::info("  changelog.hidden, changelog.sections, changelog.bug_url");
            1
        }
    }
}

/// Emit a JSON object of all effective config values to stdout.
fn list_json(cfg: &config::ProjectConfig) -> i32 {
    let default_cl = standard_changelog::ChangelogConfig::default();

    // Build a flat-ish JSON object representing the full effective config.
    let scheme = match cfg.scheme {
        config::Scheme::Semver => "semver",
        config::Scheme::Calver => "calver",
        config::Scheme::Patch => "patch",
    };

    let scopes_json: serde_json::Value = match &cfg.scopes {
        ScopesConfig::None => serde_json::Value::Null,
        ScopesConfig::Auto => serde_json::Value::String("auto".to_string()),
        ScopesConfig::List(list) => serde_json::to_value(list).unwrap(),
    };

    let hidden = cfg.changelog.hidden.as_ref().unwrap_or(&default_cl.hidden);
    let title = cfg.changelog.title.as_deref().unwrap_or(&default_cl.title);
    let sections = cfg
        .changelog
        .sections
        .as_ref()
        .unwrap_or(&default_cl.sections);
    let sections_map: serde_json::Map<String, serde_json::Value> = sections
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
        .collect();

    let obj = serde_json::json!({
        "scheme": scheme,
        "strict": cfg.strict,
        "types": cfg.types,
        "scopes": scopes_json,
        "versioning": {
            "tag_prefix": cfg.versioning.tag_prefix,
            "prerelease_tag": cfg.versioning.prerelease_tag,
            "calver_format": cfg.versioning.calver_format,
        },
        "changelog": {
            "title": title,
            "hidden": hidden,
            "sections": sections_map,
            "bug_url": cfg.changelog.bug_url,
        },
    });

    println!("{}", serde_json::to_string_pretty(&obj).unwrap());
    0
}

/// Print a plain value: text to stderr, JSON to stdout.
fn print_value(value: &str, format: OutputFormat) {
    if format == OutputFormat::Json {
        println!("{}", serde_json::to_string(value).unwrap());
    } else {
        eprintln!("{value}");
    }
}

/// Print an optional value: JSON `null` to stdout, text `"null"` to stderr.
fn print_optional_value(value: Option<&str>, format: OutputFormat) {
    match value {
        Some(v) => print_value(v, format),
        None => {
            if format == OutputFormat::Json {
                println!("null");
            } else {
                eprintln!("null");
            }
        }
    }
}

/// Print a key = value line to stderr with right-aligned source annotation.
fn print_kv(key: &str, value: &str, source: Source) {
    let lhs = format!("  {key} = {value}");
    let annotation = source.label();
    // Pad so the annotation is right-aligned at VALUE_COL total width.
    if lhs.len() < VALUE_COL {
        let padding = VALUE_COL - lhs.len();
        eprintln!("{lhs}{:width$}{annotation}", "", width = padding);
    } else {
        eprintln!("{lhs}  {annotation}");
    }
}

/// Format a `Vec<String>` as a compact JSON-style array literal.
fn format_str_list(items: &[String]) -> String {
    let inner: Vec<String> = items.iter().map(|s| format!("{s:?}")).collect();
    format!("[{}]", inner.join(", "))
}

/// Format sections `Vec<(String, String)>` as `{type: title, ...}`.
fn format_sections(sections: &[(String, String)]) -> String {
    let inner: Vec<String> = sections
        .iter()
        .map(|(k, v)| format!("{k:?}: {v:?}"))
        .collect();
    format!("{{{}}}", inner.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_str_list_empty() {
        assert_eq!(format_str_list(&[]), "[]");
    }

    #[test]
    fn format_str_list_single() {
        assert_eq!(format_str_list(&["feat".to_string()]), r#"["feat"]"#);
    }

    #[test]
    fn format_str_list_multiple() {
        let items = vec!["feat".to_string(), "fix".to_string()];
        assert_eq!(format_str_list(&items), r#"["feat", "fix"]"#);
    }

    #[test]
    fn format_sections_produces_object_notation() {
        let sections = vec![
            ("feat".to_string(), "Features".to_string()),
            ("fix".to_string(), "Bug Fixes".to_string()),
        ];
        let result = format_sections(&sections);
        assert!(result.starts_with('{'));
        assert!(result.ends_with('}'));
        assert!(result.contains("\"feat\": \"Features\""));
    }
}
