/// Generate the shim script content for a given hook name.
///
/// The shim delegates execution to `git std hooks run <hook> -- <args>`,
/// passing through any arguments git provides after `--` so that clap's
/// `#[arg(last = true)]` can capture them.
///
/// # Example
///
/// ```
/// use standard_githooks::shim::generate_shim;
///
/// let shim = generate_shim("pre-commit");
/// assert_eq!(shim, "#!/bin/bash\nexec git std hooks run pre-commit -- \"$@\"\n");
/// ```
pub fn generate_shim(hook_name: &str) -> String {
    format!("#!/bin/bash\nexec git std hooks run {hook_name} -- \"$@\"\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shim_for_pre_commit() {
        let shim = generate_shim("pre-commit");
        assert_eq!(
            shim,
            "#!/bin/bash\nexec git std hooks run pre-commit -- \"$@\"\n"
        );
    }

    #[test]
    fn shim_for_commit_msg() {
        let shim = generate_shim("commit-msg");
        assert_eq!(
            shim,
            "#!/bin/bash\nexec git std hooks run commit-msg -- \"$@\"\n"
        );
    }

    #[test]
    fn shim_for_pre_push() {
        let shim = generate_shim("pre-push");
        assert_eq!(
            shim,
            "#!/bin/bash\nexec git std hooks run pre-push -- \"$@\"\n"
        );
    }
}
