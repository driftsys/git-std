# Changelog

## Unreleased (2026-03-13)

### Features

- **changelog:** implement changelog generation with host-aware links ([fe3f819](https://github.com/driftsys/git-std/commit/fe3f819)), closes [#45](https://github.com/driftsys/git-std/issues/45)
- **commit:** add flag mode, --amend, --sign, --all, --dry-run ([d88750c](https://github.com/driftsys/git-std/commit/d88750c)), closes [#16](https://github.com/driftsys/git-std/issues/16), [#17](https://github.com/driftsys/git-std/issues/17), [#18](https://github.com/driftsys/git-std/issues/18), [#19](https://github.com/driftsys/git-std/issues/19)
- **check:** add --format json and coloured output ([15f757e](https://github.com/driftsys/git-std/commit/15f757e)), closes [#20](https://github.com/driftsys/git-std/issues/20), [#21](https://github.com/driftsys/git-std/issues/21)
- **commit:** add interactive conventional commit builder ([3775027](https://github.com/driftsys/git-std/commit/3775027)), closes [#15](https://github.com/driftsys/git-std/issues/15)
- **check:** add .versionrc parsing and --strict mode ([#86](https://github.com/driftsys/git-std/issues/86)) ([9fe82f4](https://github.com/driftsys/git-std/commit/9fe82f4)), closes [#13](https://github.com/driftsys/git-std/issues/13), [#14](https://github.com/driftsys/git-std/issues/14)
- **check:** add --file and --range options ([#85](https://github.com/driftsys/git-std/issues/85)) ([a0c967f](https://github.com/driftsys/git-std/commit/a0c967f)), closes [#11](https://github.com/driftsys/git-std/issues/11), [#12](https://github.com/driftsys/git-std/issues/12)
- **check:** implement inline commit message validation ([21a7333](https://github.com/driftsys/git-std/commit/21a7333)), closes [#10](https://github.com/driftsys/git-std/issues/10)
- **commit:** add conventional commit message parser ([3eda8a8](https://github.com/driftsys/git-std/commit/3eda8a8)), closes [#9](https://github.com/driftsys/git-std/issues/9)
- **cli:** set up clap CLI skeleton with subcommand stubs ([e2bc22c](https://github.com/driftsys/git-std/commit/e2bc22c)), closes [#2](https://github.com/driftsys/git-std/issues/2)
- **infra:** init cargo workspace layout ([de58038](https://github.com/driftsys/git-std/commit/de58038)), closes [#1](https://github.com/driftsys/git-std/issues/1)

### Documentation

- **agents:** add zero IDE warnings convention and cSpell guidance ([3557f65](https://github.com/driftsys/git-std/commit/3557f65))
- add README overview with workspace crates and spec references ([311ded0](https://github.com/driftsys/git-std/commit/311ded0))

### Refactoring

- extract 4 workspace crates from git-std ([b6de59d](https://github.com/driftsys/git-std/commit/b6de59d)), closes [#9](https://github.com/driftsys/git-std/issues/9), [#10](https://github.com/driftsys/git-std/issues/10)

### Bug Fixes

- **build:** drop OpenSSL dependency for cross-compilation ([db03bbe](https://github.com/driftsys/git-std/commit/db03bbe))
