/// All git hook types managed by git-std, in recommended install order.
pub const KNOWN_HOOKS: &[&str] = &[
    "pre-commit",
    "commit-msg",
    "pre-push",
    "post-commit",
    "prepare-commit-msg",
    "post-merge",
];

/// Generate the shim script content for a given hook name.
///
/// The shim delegates execution to `git std hooks run <hook> -- <args>`,
/// passing through any arguments git provides after `--` so that clap's
/// `#[arg(last = true)]` can capture them.
///
/// # Example
///
/// ```
/// use standard_githooks::generate_shim;
///
/// let shim = generate_shim("pre-commit");
/// assert!(shim.contains("exec git std hooks run pre-commit"));
/// ```
pub fn generate_shim(hook_name: &str) -> String {
    format!(
        "#!/bin/bash\n\
         # Managed by git-std — do not edit.\n\
         # Configure commands in .githooks/{hook_name}.hooks\n\
         exec git std hooks run {hook_name} -- \"$@\"\n"
    )
}

/// Generate the `.hooks` template file content for a given hook name.
///
/// Includes a full header comment explaining the prefix system with examples.
pub fn generate_hooks_template(hook_name: &str) -> String {
    let fix_line = if hook_name == "pre-commit" {
        "#   ~  fix       auto-format staged files and re-stage (pre-commit only)\n\
         #                uses a stash dance to safely isolate staged content\n"
    } else {
        ""
    };
    format!(
        "# git-std hooks — {hook_name}.hooks\n\
         #\n\
         # Each line is a command run during the {hook_name} hook.\n\
         # Prefix controls behavior:\n\
         #\n\
         #   !  check     run command, block commit on failure\n\
         {fix_line}\
         #   ?  advisory  run command, never block commit\n\
         #\n\
         # $@ contains the list of staged files — commands can use or ignore it.\n\
         #\n\
         # Examples:\n\
         #   ! cargo fmt --check   # fail if code is unformatted\n\
         #   ! cargo clippy        # lint workspace\n\
         #   ? cargo test          # run tests, never block commit\n\
         #\n\
         # Enable/disable this hook:\n\
         #   git std hooks enable {hook_name}\n\
         #   git std hooks disable {hook_name}\n\
         #\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shim_contains_exec_line() {
        let shim = generate_shim("pre-commit");
        assert!(shim.contains("exec git std hooks run pre-commit -- \"$@\""));
    }

    #[test]
    fn shim_has_managed_comment() {
        let shim = generate_shim("commit-msg");
        assert!(shim.contains("Managed by git-std"));
        assert!(shim.contains(".githooks/commit-msg.hooks"));
    }

    #[test]
    fn hooks_template_has_header() {
        let t = generate_hooks_template("pre-commit");
        assert!(t.contains("pre-commit.hooks"));
        assert!(t.contains("!  check"));
        assert!(t.contains("~  fix"));
        assert!(t.contains("?  advisory"));
    }

    #[test]
    fn hooks_template_no_fix_line_for_non_precommit() {
        let t = generate_hooks_template("commit-msg");
        assert!(!t.contains("~  fix"));
    }

    #[test]
    fn known_hooks_contains_standard_hooks() {
        assert!(KNOWN_HOOKS.contains(&"pre-commit"));
        assert!(KNOWN_HOOKS.contains(&"commit-msg"));
        assert!(KNOWN_HOOKS.contains(&"pre-push"));
    }
}
