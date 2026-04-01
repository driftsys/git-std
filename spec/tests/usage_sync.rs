//! Verify that every CLI flag appears in the docs.
//!
//! Runs `git-std <subcommand> --help` for each subcommand and checks
//! that every long flag (e.g. `--dry-run`) is mentioned in the docs.
//! This catches documentation drift when new flags are added to the
//! CLI but not reflected in the documentation.

use assert_cmd::Command;

/// Flags that are ubiquitous or intentionally undocumented.
const SKIP_FLAGS: &[&str] = &["--help", "--version", "--color"];

/// Subcommands whose flags should all appear in the docs.
const SUBCOMMANDS: &[&str] = &["commit", "lint", "bump", "changelog"];

fn git_std() -> Command {
    Command::cargo_bin("git-std").expect("binary git-std not found")
}

/// Extract long flags (e.g. `--dry-run`) from help output.
fn extract_flags(help: &str) -> Vec<String> {
    let mut flags = Vec::new();
    for line in help.lines() {
        let trimmed = line.trim();
        // clap help lines with flags look like:
        //   --flag-name <VALUE>
        //   -s, --flag-name
        for word in trimmed.split_whitespace() {
            if word.starts_with("--") {
                let clean = word.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '-');
                if !clean.is_empty() {
                    flags.push(clean.to_string());
                }
                break;
            }
        }
    }
    flags
}

#[test]
fn usage_md_documents_all_flags() {
    let docs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/USAGE.md"))
        .expect("failed to read docs/USAGE.md");

    let mut missing: Vec<(String, String)> = Vec::new();

    for subcmd in SUBCOMMANDS {
        let output = git_std()
            .args([subcmd, "--help"])
            .output()
            .unwrap_or_else(|e| panic!("failed to run git-std {subcmd} --help: {e}"));

        let help = String::from_utf8_lossy(&output.stdout);
        let flags = extract_flags(&help);

        for flag in &flags {
            if SKIP_FLAGS.contains(&flag.as_str()) {
                continue;
            }
            let pattern = format!("`{flag}");
            if !docs.contains(&pattern) {
                missing.push((subcmd.to_string(), flag.clone()));
            }
        }
    }

    // Also check that all subcommands from top-level help are mentioned.
    let output = git_std()
        .arg("--help")
        .output()
        .expect("failed to run git-std --help");
    let help = String::from_utf8_lossy(&output.stdout);
    let mut missing_cmds: Vec<String> = Vec::new();
    let mut in_commands = false;
    for line in help.lines() {
        if line.starts_with("Commands:") {
            in_commands = true;
            continue;
        }
        if in_commands {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some(cmd) = trimmed.split_whitespace().next()
                && cmd != "help"
                && !docs.contains(&format!("git std {cmd}"))
            {
                missing_cmds.push(cmd.to_string());
            }
        }
    }

    let mut errors = String::new();
    if !missing.is_empty() {
        errors.push_str("Flags missing from docs:\n");
        for (cmd, flag) in &missing {
            errors.push_str(&format!("  git std {cmd} {flag}\n"));
        }
    }
    if !missing_cmds.is_empty() {
        errors.push_str("Subcommands missing from docs:\n");
        for cmd in &missing_cmds {
            errors.push_str(&format!("  git std {cmd}\n"));
        }
    }

    assert!(errors.is_empty(), "\n{errors}");
}
