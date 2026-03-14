# Changelog

## 0.1.0 (2026-03-14)

### Features

- **bump:** implement version bump workflow with --dry-run support ([#92])
  ([d4747e2]), closes [#47], closes #48
- **changelog:** implement changelog generation ([#90]) ([9484fe7]), closes
  [#45]
- **commit:** add flag mode, --amend, --sign, --all, --dry-run ([d88750c]), refs
  [#16], [#17], [#18], [#19]
- **check:** add --format json and coloured output ([15f757e]), refs [#20],
  [#21]
- **commit:** add interactive conventional commit builder ([3775027]), refs
  [#15]
- **check:** add .versionrc parsing and --strict mode ([#86]) ([9fe82f4]),
  closes [#13], [#14]
- **check:** add --file and --range options ([#85]) ([a0c967f]), closes [#11],
  [#12]
- **check:** implement inline commit message validation ([21a7333]), closes
  [#10]
- **commit:** add conventional commit message parser ([3eda8a8]), closes [#9]
- **cli:** set up clap CLI skeleton with subcommand stubs ([e2bc22c]), closes
  [#2]
- **infra:** init cargo workspace layout ([de58038]), closes [#1]

### Documentation

- improve public API rustdoc with examples ([b65ee7d])
- add README and CONTRIBUTING ([#91]) ([598d816])
- **agents:** add zero IDE warnings convention and cSpell guidance ([3557f65])
- add README overview with workspace crates and spec references ([311ded0])

### Refactoring

- move logic to library crates and organize CLI modules ([#93]) ([1dd8cfa])
- extract 4 workspace crates from git-std ([b6de59d]), closes [#9], [#10]

### Bug Fixes

- **bump:** support workspace manifests in Cargo.toml discovery ([0daad4c])
- **ci:** use workflow conditionals instead of shell if for Windows compat
  ([8809661])
- **ci:** use cross for aarch64-unknown-linux-musl build ([4e55a3e])
- **ci:** use musl cross-compiler for aarch64-unknown-linux-musl ([d9ee886])
- **build:** drop OpenSSL dependency for cross-compilation ([db03bbe])

[d4747e2]: https://github.com/driftsys/git-std/commit/d4747e2
[#92]: https://github.com/driftsys/git-std/issues/92
[#47]: https://github.com/driftsys/git-std/issues/47
[9484fe7]: https://github.com/driftsys/git-std/commit/9484fe7
[#90]: https://github.com/driftsys/git-std/issues/90
[#45]: https://github.com/driftsys/git-std/issues/45
[d88750c]: https://github.com/driftsys/git-std/commit/d88750c
[#16]: https://github.com/driftsys/git-std/issues/16
[#17]: https://github.com/driftsys/git-std/issues/17
[#18]: https://github.com/driftsys/git-std/issues/18
[#19]: https://github.com/driftsys/git-std/issues/19
[15f757e]: https://github.com/driftsys/git-std/commit/15f757e
[#20]: https://github.com/driftsys/git-std/issues/20
[#21]: https://github.com/driftsys/git-std/issues/21
[3775027]: https://github.com/driftsys/git-std/commit/3775027
[#15]: https://github.com/driftsys/git-std/issues/15
[9fe82f4]: https://github.com/driftsys/git-std/commit/9fe82f4
[#86]: https://github.com/driftsys/git-std/issues/86
[#13]: https://github.com/driftsys/git-std/issues/13
[#14]: https://github.com/driftsys/git-std/issues/14
[a0c967f]: https://github.com/driftsys/git-std/commit/a0c967f
[#85]: https://github.com/driftsys/git-std/issues/85
[#11]: https://github.com/driftsys/git-std/issues/11
[#12]: https://github.com/driftsys/git-std/issues/12
[21a7333]: https://github.com/driftsys/git-std/commit/21a7333
[#10]: https://github.com/driftsys/git-std/issues/10
[3eda8a8]: https://github.com/driftsys/git-std/commit/3eda8a8
[#9]: https://github.com/driftsys/git-std/issues/9
[e2bc22c]: https://github.com/driftsys/git-std/commit/e2bc22c
[#2]: https://github.com/driftsys/git-std/issues/2
[de58038]: https://github.com/driftsys/git-std/commit/de58038
[#1]: https://github.com/driftsys/git-std/issues/1
[b65ee7d]: https://github.com/driftsys/git-std/commit/b65ee7d
[598d816]: https://github.com/driftsys/git-std/commit/598d816
[#91]: https://github.com/driftsys/git-std/issues/91
[3557f65]: https://github.com/driftsys/git-std/commit/3557f65
[311ded0]: https://github.com/driftsys/git-std/commit/311ded0
[1dd8cfa]: https://github.com/driftsys/git-std/commit/1dd8cfa
[#93]: https://github.com/driftsys/git-std/issues/93
[b6de59d]: https://github.com/driftsys/git-std/commit/b6de59d
[0daad4c]: https://github.com/driftsys/git-std/commit/0daad4c
[8809661]: https://github.com/driftsys/git-std/commit/8809661
[4e55a3e]: https://github.com/driftsys/git-std/commit/4e55a3e
[d9ee886]: https://github.com/driftsys/git-std/commit/d9ee886
[db03bbe]: https://github.com/driftsys/git-std/commit/db03bbe
