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
- `matches_any` — check if staged files match a glob pattern
- `default_mode` — get the default execution mode for a hook
- `substitute_msg` — replace `{msg}` tokens in commands
- `generate_shim` — generate a shim script for a hook

## Hook file format

Each `.githooks/<hook>.hooks` file contains one command per
line with an optional prefix and trailing glob:

```text
# Comment
[prefix]command [arguments] [glob]
```

Prefixes: _(none)_ = hook default, `!` = fail fast,
`?` = advisory.

## Part of git-std

This crate is one of four libraries powering [git-std][git-std],
a single binary for conventional commits, versioning, changelog,
and git hooks.

## License

MIT

[git-std]: https://github.com/driftsys/git-std
