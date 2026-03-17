# Changelog

## [0.3.0] (2026-03-17)

### Bug Fixes

- **docs:** remove stray character in SPEC.md table ([1bde589])
- **spec:** add missing snapshot files for patch and stable bump tests
  ([df36f45])
- improve error messages and diagnostics across CLI ([1337200]), closes [#67]
- **spec:** simplify changelog range snapshot for parallel test stability
  ([6ea2dac])
- **spec:** use flexible matching for changelog range snapshot ([239d000])
- **spec:** correct changelog range snapshot section order ([d8c8ad7])
- **ci:** use cargo install for mdbook instead of broken URL ([3f79092])

### Documentation

- sync SPEC, USAGE, and README with current CLI ([3d0f36a])
- defer org-wide policies to driftsys/.github ([f498fca])
- add AI policy section to CONTRIBUTING.md ([eec0a62])
- replace docs.rs badge with user guide badge for binary crate ([a666bbc])
- add badges, book links, and documentation metadata ([6696f5d])
- move issue model to CONTRIBUTING.md, trim AGENTS.md ([cae0d4b])

### Refactoring

- **bump:** re-extract finalize_bump helper with ui module ([58570e9]), closes
  [#128]
- add shared ui module for consistent CLI output ([8f837b1]), closes [#132]
- **bump:** extract shared finalize_bump helper ([5c34c72]), closes [#128]
- use thiserror for error derives in library crates ([32d89da]), closes [#133]
- **spec:** extract inline assertions to snapshot files ([6bd7149])

### Features

- add shell completions for bash, zsh, and fish ([9948c9b])
- **git:** replace git2 with git CLI subprocess calls ([#177]) ([213af52]),
  closes [#134]
- auto-discover scopes from workspace directory layout ([39c70ab]), closes [#72]
- validate calver_format at config parse time ([a7313cd]), closes [#124]
- **bump:** align patch scheme error message with spec ([5ebe5dc]), closes
  [#138]
- **bump:** implement --stable flag for creating patch-only branches
  ([3f6f623]), closes [#139]
- **bump:** implement scheme = "patch" for patch-only branches ([32111a7]),
  closes [#138]
- **hooks:** skip hook execution when GIT_STD_SKIP_HOOKS is set ([78bb445]),
  closes [#73]

[0.3.0]: https://github.com/driftsys/git-std/compare/v0.2.0...v0.3.0
[1bde589]: https://github.com/driftsys/git-std/commit/1bde589
[df36f45]: https://github.com/driftsys/git-std/commit/df36f45
[1337200]: https://github.com/driftsys/git-std/commit/1337200
[#67]: https://github.com/driftsys/git-std/issues/67
[6ea2dac]: https://github.com/driftsys/git-std/commit/6ea2dac
[239d000]: https://github.com/driftsys/git-std/commit/239d000
[d8c8ad7]: https://github.com/driftsys/git-std/commit/d8c8ad7
[3f79092]: https://github.com/driftsys/git-std/commit/3f79092
[3d0f36a]: https://github.com/driftsys/git-std/commit/3d0f36a
[f498fca]: https://github.com/driftsys/git-std/commit/f498fca
[eec0a62]: https://github.com/driftsys/git-std/commit/eec0a62
[a666bbc]: https://github.com/driftsys/git-std/commit/a666bbc
[6696f5d]: https://github.com/driftsys/git-std/commit/6696f5d
[cae0d4b]: https://github.com/driftsys/git-std/commit/cae0d4b
[58570e9]: https://github.com/driftsys/git-std/commit/58570e9
[#128]: https://github.com/driftsys/git-std/issues/128
[8f837b1]: https://github.com/driftsys/git-std/commit/8f837b1
[#132]: https://github.com/driftsys/git-std/issues/132
[5c34c72]: https://github.com/driftsys/git-std/commit/5c34c72
[32d89da]: https://github.com/driftsys/git-std/commit/32d89da
[#133]: https://github.com/driftsys/git-std/issues/133
[6bd7149]: https://github.com/driftsys/git-std/commit/6bd7149
[9948c9b]: https://github.com/driftsys/git-std/commit/9948c9b
[213af52]: https://github.com/driftsys/git-std/commit/213af52
[#177]: https://github.com/driftsys/git-std/issues/177
[#134]: https://github.com/driftsys/git-std/issues/134
[39c70ab]: https://github.com/driftsys/git-std/commit/39c70ab
[#72]: https://github.com/driftsys/git-std/issues/72
[a7313cd]: https://github.com/driftsys/git-std/commit/a7313cd
[#124]: https://github.com/driftsys/git-std/issues/124
[5ebe5dc]: https://github.com/driftsys/git-std/commit/5ebe5dc
[#138]: https://github.com/driftsys/git-std/issues/138
[3f6f623]: https://github.com/driftsys/git-std/commit/3f6f623
[#139]: https://github.com/driftsys/git-std/issues/139
[32111a7]: https://github.com/driftsys/git-std/commit/32111a7
[78bb445]: https://github.com/driftsys/git-std/commit/78bb445
[#73]: https://github.com/driftsys/git-std/issues/73

## [0.2.0] (2026-03-16)

### Documentation

- update USAGE.md and READMEs for current features ([9239f32])
- document calver tag return type rationale ([586082b]), closes [#129]
- fix collect mode terminology in hooks spec ([3a5e995]), closes [#112]
- update AGENTS.md and SPEC.md for current state ([701c904])
- allow org-level epic references in issue model ([51caef6])
- standardize issue model in AGENTS.md ([d32edd2])
- update merge strategy to merge commits for traceability ([5d7ae77])
- update merge strategy to merge commits for traceability ([b022c2d])
- add isolation and PR review guidelines to AGENTS.md ([027c9bb])
- allow standard-version crate to perform file I/O ([#107]) ([f2889b8])
- **bump:** add multi-ecosystem version file support to spec ([#95]) ([b9af904])
- add per-crate READMEs and streamline root README ([b3fd36b])
- fix inaccuracies across all documentation ([8761337])

### Features

- wire calver into bump, add changelog --range, integration tests ([97aafd7])
- implement calver, changelog --range, and release cycle tests ([#121])
  ([c169f16]), closes [#44], closes #54, closes #56, closes #57
- **bump:** add regex engine for custom [[version_files]] ([#118]) ([0ebc583]),
  closes [#103]
- **bump:** wire multi-ecosystem version files into bump workflow ([#117])
  ([5c1a05f]), closes [#104]
- **bump:** add text-based version file engines ([#111]) ([6b2bd6f]), closes
  [#100], closes #101, closes #102
- **bump:** add JSON version file engines for package.json and deno.json
  ([#110]) ([adca4cd]), closes [#98]
- **bump:** add pyproject.toml version file engine ([#109]) ([3be8c42]), closes
  [#99]
- **hooks:** implement hooks run ([#22]-[#28]) ([#106]) ([b28d51c]), closes
  [#22], closes #23, closes #24, closes #25, closes #26, closes #27, closes #28,
  [#29], closes #30, closes #31, [#32], closes #33, closes #34, closes #35
- **bump:** add VersionFile trait and refactor Cargo.toml engine ([#108])
  ([f4195a5]), closes [#97]

### Refactoring

- **hooks:** improve API consistency and extract helpers ([#116]) ([a14a669]),
  closes [#98]

### Bug Fixes

- **bump:** warn on system clock failure in calver date ([eb5fcee]), closes
  [#131]
- **bump:** address calver review findings ([b5c87bc])
- **bump:** address review findings from multi-ecosystem version files ([#122])
  ([e670128])
- **bump:** sync Cargo.lock after version update ([ee60541])

[0.2.0]: https://github.com/driftsys/git-std/compare/v0.1.0...v0.2.0
[9239f32]: https://github.com/driftsys/git-std/commit/9239f32
[586082b]: https://github.com/driftsys/git-std/commit/586082b
[#129]: https://github.com/driftsys/git-std/issues/129
[3a5e995]: https://github.com/driftsys/git-std/commit/3a5e995
[#112]: https://github.com/driftsys/git-std/issues/112
[701c904]: https://github.com/driftsys/git-std/commit/701c904
[51caef6]: https://github.com/driftsys/git-std/commit/51caef6
[d32edd2]: https://github.com/driftsys/git-std/commit/d32edd2
[5d7ae77]: https://github.com/driftsys/git-std/commit/5d7ae77
[b022c2d]: https://github.com/driftsys/git-std/commit/b022c2d
[027c9bb]: https://github.com/driftsys/git-std/commit/027c9bb
[f2889b8]: https://github.com/driftsys/git-std/commit/f2889b8
[#107]: https://github.com/driftsys/git-std/issues/107
[b9af904]: https://github.com/driftsys/git-std/commit/b9af904
[#95]: https://github.com/driftsys/git-std/issues/95
[b3fd36b]: https://github.com/driftsys/git-std/commit/b3fd36b
[8761337]: https://github.com/driftsys/git-std/commit/8761337
[97aafd7]: https://github.com/driftsys/git-std/commit/97aafd7
[c169f16]: https://github.com/driftsys/git-std/commit/c169f16
[#121]: https://github.com/driftsys/git-std/issues/121
[#44]: https://github.com/driftsys/git-std/issues/44
[0ebc583]: https://github.com/driftsys/git-std/commit/0ebc583
[#118]: https://github.com/driftsys/git-std/issues/118
[#103]: https://github.com/driftsys/git-std/issues/103
[5c1a05f]: https://github.com/driftsys/git-std/commit/5c1a05f
[#117]: https://github.com/driftsys/git-std/issues/117
[#104]: https://github.com/driftsys/git-std/issues/104
[6b2bd6f]: https://github.com/driftsys/git-std/commit/6b2bd6f
[#111]: https://github.com/driftsys/git-std/issues/111
[#100]: https://github.com/driftsys/git-std/issues/100
[adca4cd]: https://github.com/driftsys/git-std/commit/adca4cd
[#110]: https://github.com/driftsys/git-std/issues/110
[#98]: https://github.com/driftsys/git-std/issues/98
[3be8c42]: https://github.com/driftsys/git-std/commit/3be8c42
[#109]: https://github.com/driftsys/git-std/issues/109
[#99]: https://github.com/driftsys/git-std/issues/99
[b28d51c]: https://github.com/driftsys/git-std/commit/b28d51c
[#22]: https://github.com/driftsys/git-std/issues/22
[#28]: https://github.com/driftsys/git-std/issues/28
[#106]: https://github.com/driftsys/git-std/issues/106
[#29]: https://github.com/driftsys/git-std/issues/29
[#32]: https://github.com/driftsys/git-std/issues/32
[f4195a5]: https://github.com/driftsys/git-std/commit/f4195a5
[#108]: https://github.com/driftsys/git-std/issues/108
[#97]: https://github.com/driftsys/git-std/issues/97
[a14a669]: https://github.com/driftsys/git-std/commit/a14a669
[#116]: https://github.com/driftsys/git-std/issues/116
[eb5fcee]: https://github.com/driftsys/git-std/commit/eb5fcee
[#131]: https://github.com/driftsys/git-std/issues/131
[b5c87bc]: https://github.com/driftsys/git-std/commit/b5c87bc
[e670128]: https://github.com/driftsys/git-std/commit/e670128
[#122]: https://github.com/driftsys/git-std/issues/122
[ee60541]: https://github.com/driftsys/git-std/commit/ee60541

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

### Documentation

- improve public API rustdoc with examples ([b65ee7d])
- add README and CONTRIBUTING ([#91]) ([598d816])
- **agents:** add zero IDE warnings convention and cSpell guidance ([3557f65])
- add README overview with workspace crates and spec references ([311ded0])

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
[1dd8cfa]: https://github.com/driftsys/git-std/commit/1dd8cfa
[#93]: https://github.com/driftsys/git-std/issues/93
[b6de59d]: https://github.com/driftsys/git-std/commit/b6de59d
[0daad4c]: https://github.com/driftsys/git-std/commit/0daad4c
[8809661]: https://github.com/driftsys/git-std/commit/8809661
[4e55a3e]: https://github.com/driftsys/git-std/commit/4e55a3e
[d9ee886]: https://github.com/driftsys/git-std/commit/d9ee886
[db03bbe]: https://github.com/driftsys/git-std/commit/db03bbe
[b65ee7d]: https://github.com/driftsys/git-std/commit/b65ee7d
[598d816]: https://github.com/driftsys/git-std/commit/598d816
[#91]: https://github.com/driftsys/git-std/issues/91
[3557f65]: https://github.com/driftsys/git-std/commit/3557f65
[311ded0]: https://github.com/driftsys/git-std/commit/311ded0
