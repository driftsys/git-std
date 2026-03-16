# standard-changelog

[![crates.io](https://img.shields.io/crates/v/standard-changelog.svg)](https://crates.io/crates/standard-changelog)
[![docs.rs](https://docs.rs/standard-changelog/badge.svg)](https://docs.rs/standard-changelog)

Changelog generation from conventional commits.

Groups parsed commits by type, renders markdown sections, and
manages `CHANGELOG.md` files. Pure library — no I/O, no git
operations, no terminal output.

## Entry points

- `build_release` — parse raw commits into a `VersionRelease`
- `render` — render multiple releases into a full changelog
- `render_version` — render a single version section
- `prepend_release` — splice a new release into an existing
  changelog

## Example

```rust
use standard_changelog::{
    build_release, render, ChangelogConfig, RepoHost,
};

let commits = vec![
    ("abc1234", "feat(auth): add OAuth2 PKCE flow"),
    ("def5678", "fix: handle expired tokens"),
];

let config = ChangelogConfig::default();
let mut release =
    build_release(&commits, "1.0.0", None, &config).unwrap();
release.date = "2026-03-14".to_string();

let host = RepoHost::Unknown;
let changelog = render(&[release], &config, &host);
assert!(changelog.contains("## 1.0.0 (2026-03-14)"));
```

## Part of git-std

This crate is one of four libraries powering [git-std][git-std],
a single binary for conventional commits, versioning, changelog,
and git hooks.

## License

MIT

[git-std]: https://github.com/driftsys/git-std
