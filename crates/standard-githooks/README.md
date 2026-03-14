# standard-githooks

Git hooks file format parsing, shim generation, and execution
model.

Owns the `.githooks/<hook>.hooks` file format. Can read/write
hook files and generate shim scripts. Does not execute commands,
run git operations, or produce terminal output.

## Part of git-std

This crate is one of four libraries powering [git-std][git-std],
a single binary for conventional commits, versioning, changelog,
and git hooks.

## License

MIT

[git-std]: https://github.com/driftsys/git-std
