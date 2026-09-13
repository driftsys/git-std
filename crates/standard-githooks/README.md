# standard-githooks

[![crates.io](https://img.shields.io/crates/v/standard-githooks.svg)](https://crates.io/crates/standard-githooks)
[![docs.rs](https://docs.rs/standard-githooks/badge.svg)](https://docs.rs/standard-githooks)

Git hooks file format parsing, shim generation, and execution
model.

Owns the `.githooks/<hook>.hooks` file format. Can read/write
hook files and generate shim scripts. Does not execute commands,
run git operations, or produce terminal output.

## Main entry points

- `parse` — parse hook file content into a list of commands
- `split_delete_marker` — identify the optional pre-push delete marker
- `matches_any` — check if staged files match a glob pattern
- `is_deletion_only` — classify Git's `pre-push` ref input
- `default_mode` — get the default execution mode for a hook
- `substitute_msg` — replace `{msg}` tokens in commands
- `generate_shim` — generate a shim script for a hook

## Hook file format

Each `.githooks/<hook>.hooks` file contains one command per
line with an optional prefix, delete marker, and trailing glob:

```text
# Comment
[prefix] [delete] command [arguments] [glob]
```

Prefixes: _(none)_ = hook default, `!` = fail fast,
`?` = advisory.

For `pre-push`, unmarked commands skip deletion-only pushes. Add `[delete]`
after the optional prefix when a command should also run for them.

## Part of git-std

This crate is one of four libraries powering [git-std][git-std],
a single binary for conventional commits, versioning, changelog,
and git hooks.

## License

MIT

[git-std]: https://github.com/driftsys/git-std
