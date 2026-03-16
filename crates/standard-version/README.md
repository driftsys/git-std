# standard-version

Version bump calculation from conventional commits — supports
semver and calendar versioning (calver).

Also provides the `VersionFile` trait for ecosystem-specific
version file detection and updating (Cargo.toml, package.json,
pyproject.toml, pubspec.yaml, gradle.properties, VERSION).

## Entry points

- `determine_bump` — analyse commits and return the bump level
- `apply_bump` — apply a bump level to a semver version
- `apply_prerelease` — bump with a pre-release tag (e.g. `rc.0`)
- `replace_version_in_toml` — update the version in a TOML string
- `update_version_files` — detect and update version files
- `calver::next_version` — compute next calver version from date

## Example

```rust
use standard_version::{determine_bump, apply_bump, BumpLevel};

let commits = vec![
    standard_commit::parse("feat: add login").unwrap(),
    standard_commit::parse("fix: handle timeout").unwrap(),
];

let level = determine_bump(&commits).unwrap();
assert_eq!(level, BumpLevel::Minor);

let current = semver::Version::new(1, 2, 3);
let next = apply_bump(&current, level);
assert_eq!(next, semver::Version::new(1, 3, 0));
```

## Part of git-std

This crate is one of four libraries powering [git-std][git-std],
a single binary for conventional commits, versioning, changelog,
and git hooks.

## License

MIT

[git-std]: https://github.com/driftsys/git-std
