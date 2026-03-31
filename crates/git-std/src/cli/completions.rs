use clap_complete::Shell;

/// Shell snippet that enables `git std …` tab-completion.
///
/// `clap_complete::generate` registers completions for the standalone
/// `git-std` binary.  Each shell's git-completion framework uses a
/// different mechanism for custom sub-commands; we append the right
/// wrapper so that `git std <TAB>` works too.
pub fn git_subcommand_wrapper(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => GIT_SUBCMD_BASH,
        Shell::Zsh => GIT_SUBCMD_ZSH,
        Shell::Fish => GIT_SUBCMD_FISH,
        _ => "",
    }
}

/// Bash: git's completion framework calls `_git_<subcmd>` for custom
/// sub-commands.  We translate `COMP_WORDS` from `("git" "std" …)` to
/// `("git-std" …)` and delegate to the clap-generated `_git-std`.
const GIT_SUBCMD_BASH: &str = r#"
# --- git subcommand wrapper -------------------------------------------
# Enable completion for "git std" (git subcommand invocation).
# Git's bash completion calls _git_<subcmd> for custom subcommands.
_git_std() {
    local _save_words=("${COMP_WORDS[@]}")
    local _save_cword=$COMP_CWORD
    # Merge "git std" into "git-std" so _git-std sees the expected layout.
    COMP_WORDS=("git-std" "${COMP_WORDS[@]:2}")
    (( COMP_CWORD -= 1 ))
    local _cur="${COMP_WORDS[$COMP_CWORD]}"
    local _prev="${COMP_WORDS[$((COMP_CWORD > 0 ? COMP_CWORD - 1 : 0))]}"
    _git-std "git-std" "$_cur" "$_prev"
    COMP_WORDS=("${_save_words[@]}")
    COMP_CWORD=$_save_cword
}
"#;

/// Zsh: register `std` as a git user-command so `_git` delegates to the
/// already-defined `_git-std` completion function.
const GIT_SUBCMD_ZSH: &str = r#"
# --- git subcommand wrapper -------------------------------------------
# Register "std" as a git user-command so "git std <TAB>" triggers _git-std.
zstyle ':completion:*:*:git:*' user-commands std:'Conventional commit standards'
"#;

/// Fish: register `std` as a git subcommand and list top-level
/// sub-commands.  Deeper option completion is handled by the standalone
/// `git-std` completions above.
const GIT_SUBCMD_FISH: &str = r#"
# --- git subcommand wrapper -------------------------------------------
# Register "std" as a git subcommand for "git std <TAB>" completion.
complete -f -c git -n __fish_git_needs_command -a std -d 'Conventional commit standards'
complete -f -c git -n '__fish_git_using_command std' -a commit -d 'Interactive conventional commit builder'
complete -f -c git -n '__fish_git_using_command std' -a check -d 'Validate commit messages'
complete -f -c git -n '__fish_git_using_command std' -a bump -d 'Version bump, changelog, commit, and tag'
complete -f -c git -n '__fish_git_using_command std' -a changelog -d 'Generate a changelog'
complete -f -c git -n '__fish_git_using_command std' -a bootstrap -d 'Post-clone environment setup'
complete -f -c git -n '__fish_git_using_command std' -a hooks -d 'Git hooks management'
complete -f -c git -n '__fish_git_using_command std' -a config -d 'Inspect git-std configuration'
complete -f -c git -n '__fish_git_using_command std' -a doctor -d 'Run health checks'
complete -f -c git -n '__fish_git_using_command std' -a completions -d 'Generate shell completions'
"#;
