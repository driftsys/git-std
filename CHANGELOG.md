# Changelog

## [0.9.0] (2026-03-28)

### Features

- support calver scheme in monorepo mode ([#385]) ([f32d014])
- support calver scheme in monorepo mode ([55832f9]), fixes 377 item 3.
- **cli:** add monorepo CLI polish and changelog command ([#375]) ([eb682aa])
- **cli:** add monorepo CLI polish and changelog command ([d8578d8]), closes
  [#367]
- **bump:** implement per-package apply for monorepo versioning ([#374])
  ([909d3ac])
- **bump:** implement per-package apply for monorepo versioning ([1aaef7b]),
  closes [#366]
- **bump:** add dependency cascade for monorepo versioning ([#373]) ([82b0758])
- **bump:** add dependency cascade for monorepo versioning ([f71b0f8]), closes
  [#365]
- **bump:** add per-package monorepo version plan ([#372]) ([5c783c8])
- add per-package version planning for monorepo bump ([f7d895f]), closes [#364]
- **config:** add workspace auto-discovery ([#371]) ([5a3ca6a])
- **config:** add workspace auto-discovery ([bc11d47]), closes [#362]
- **git:** add path-filtered commit walking ([#370]) ([f76f09e])
- **git:** add path-filtered commit walking ([e50fa10]), closes [#363]
- **config:** add monorepo flag, packages schema, and tag_template ([#369])
  ([85756f2])
- **config:** add monorepo flag, packages schema, and tag_template ([4fb09a1]),
  closes [#361]

### Refactoring

- extract finalize_monorepo_bump into focused helpers ([#382]) ([c7857d9])
- extract finalize_monorepo_bump into focused helpers ([9ac5592]), fixes 377
  items 6 and 7.
- replace unwrap and is_some anti-patterns in monorepo ([#378]) ([3bff7a2])
- replace unwrap and is_some anti-patterns in monorepo ([7be739d]), fixes 377
  items 5 and 6.
- collect tags once in monorepo planning ([#381]) ([38fa62a])
- collect tags once in monorepo planning ([2f480d5]), fixes 377 item 4.
- avoid redundant package discovery in resolved_scopes ([#384]) ([ec68970])
- avoid redundant package discovery in resolved_scopes ([eba1685]), fixes 377
  item 8.
- **doctor:** extract check_line helper, fix hint rendering, deepen tests
  ([#359]) ([f621b1a])

### Documentation

- add monorepo versioning documentation ([#386]) ([d49227d])
- add monorepo versioning documentation ([3e33299])

### Bug Fixes

- use per-package changelog config in monorepo bump ([#380]) ([e941f76])
- use per-package changelog config in monorepo bump ([7b77014]), fixes 377 item
  2.
- use per-package version_files in monorepo bump ([#379]) ([cd64224])
- use per-package version_files in monorepo bump ([82fed7d]), fixes 377 item 1.

[0.9.0]: https://github.com/driftsys/git-std/compare/v0.8.0...v0.9.0
[f32d014]: https://github.com/driftsys/git-std/commit/f32d014
[#385]: https://github.com/driftsys/git-std/issues/385
[55832f9]: https://github.com/driftsys/git-std/commit/55832f9
[eb682aa]: https://github.com/driftsys/git-std/commit/eb682aa
[#375]: https://github.com/driftsys/git-std/issues/375
[d8578d8]: https://github.com/driftsys/git-std/commit/d8578d8
[#367]: https://github.com/driftsys/git-std/issues/367
[909d3ac]: https://github.com/driftsys/git-std/commit/909d3ac
[#374]: https://github.com/driftsys/git-std/issues/374
[1aaef7b]: https://github.com/driftsys/git-std/commit/1aaef7b
[#366]: https://github.com/driftsys/git-std/issues/366
[82b0758]: https://github.com/driftsys/git-std/commit/82b0758
[#373]: https://github.com/driftsys/git-std/issues/373
[f71b0f8]: https://github.com/driftsys/git-std/commit/f71b0f8
[#365]: https://github.com/driftsys/git-std/issues/365
[5c783c8]: https://github.com/driftsys/git-std/commit/5c783c8
[#372]: https://github.com/driftsys/git-std/issues/372
[f7d895f]: https://github.com/driftsys/git-std/commit/f7d895f
[#364]: https://github.com/driftsys/git-std/issues/364
[5a3ca6a]: https://github.com/driftsys/git-std/commit/5a3ca6a
[#371]: https://github.com/driftsys/git-std/issues/371
[bc11d47]: https://github.com/driftsys/git-std/commit/bc11d47
[#362]: https://github.com/driftsys/git-std/issues/362
[f76f09e]: https://github.com/driftsys/git-std/commit/f76f09e
[#370]: https://github.com/driftsys/git-std/issues/370
[e50fa10]: https://github.com/driftsys/git-std/commit/e50fa10
[#363]: https://github.com/driftsys/git-std/issues/363
[85756f2]: https://github.com/driftsys/git-std/commit/85756f2
[#369]: https://github.com/driftsys/git-std/issues/369
[4fb09a1]: https://github.com/driftsys/git-std/commit/4fb09a1
[#361]: https://github.com/driftsys/git-std/issues/361
[c7857d9]: https://github.com/driftsys/git-std/commit/c7857d9
[#382]: https://github.com/driftsys/git-std/issues/382
[9ac5592]: https://github.com/driftsys/git-std/commit/9ac5592
[3bff7a2]: https://github.com/driftsys/git-std/commit/3bff7a2
[#378]: https://github.com/driftsys/git-std/issues/378
[7be739d]: https://github.com/driftsys/git-std/commit/7be739d
[38fa62a]: https://github.com/driftsys/git-std/commit/38fa62a
[#381]: https://github.com/driftsys/git-std/issues/381
[2f480d5]: https://github.com/driftsys/git-std/commit/2f480d5
[ec68970]: https://github.com/driftsys/git-std/commit/ec68970
[#384]: https://github.com/driftsys/git-std/issues/384
[eba1685]: https://github.com/driftsys/git-std/commit/eba1685
[f621b1a]: https://github.com/driftsys/git-std/commit/f621b1a
[#359]: https://github.com/driftsys/git-std/issues/359
[d49227d]: https://github.com/driftsys/git-std/commit/d49227d
[#386]: https://github.com/driftsys/git-std/issues/386
[3e33299]: https://github.com/driftsys/git-std/commit/3e33299
[e941f76]: https://github.com/driftsys/git-std/commit/e941f76
[#380]: https://github.com/driftsys/git-std/issues/380
[7b77014]: https://github.com/driftsys/git-std/commit/7b77014
[cd64224]: https://github.com/driftsys/git-std/commit/cd64224
[#379]: https://github.com/driftsys/git-std/issues/379
[82fed7d]: https://github.com/driftsys/git-std/commit/82fed7d

## [0.8.0] (2026-03-28)

### Features

- **standard-version:** auto-detect project.toml, project.json, project.yaml
  ([841a626])

[0.8.0]: https://github.com/driftsys/git-std/compare/v0.7.0...v0.8.0
[841a626]: https://github.com/driftsys/git-std/commit/841a626

## [0.7.0] (2026-03-28)

### Bug Fixes

- **docs:** add missing revert to default types in schema and CONFIG ([34bd5ab])
- **doctor:** accept absolute path for blame.ignoreRevsFile ([#340]) ([68e7b33])
- **git-std:** address K1 review issues in ecosystem trait ([ee98800])
- **doctor:** fix run_json exit code, lfs filter match, and stale comment
  ([0a939cf])
- **doctor:** fix hooksPath absolute-path comparison and run_json exit code
  ([7877bdf])
- **doctor:** inline dead_code justification and fix run_json exit code
  ([0ba7163])
- **doctor:** use crate::git::workdir import path ([c677616])
- **git-std:** fail fast in hooks install without TTY ([#316]) ([d3f3d2e]),
  closes [#316]
- **git-std:** resolve hooks and bootstrap paths from repo root ([#329])
  ([add2ffe]), closes [#318], [#317]
- **git-std:** write config get JSON null to stdout ([#315]) ([3496a5f]), closes
  [#315]
- **release:** surface crates.io publish failures and improve version check
  ([6b9d2b7])

### Features

- **git-std:** introduce Ecosystem trait for version bump orchestration
  ([526cf92])
- **git-std:** --format json for git std doctor ([e37ccd8])
- **git-std:** config health checks ([2327d66]), closes [#325]
- **doctor:** bootstrap health checks ([3cc9bf0]), closes [#324]
- **git-std:** hooks health checks ([9b8b2cb])
- **doctor:** git std doctor skeleton ([52a5785])
- **git-std:** install man pages and shell completions on install ([63d02f6])
- **release:** add aarch64-linux-musl and Windows release targets ([0fa6925])

[0.7.0]: https://github.com/driftsys/git-std/compare/v0.6.0...v0.7.0
[34bd5ab]: https://github.com/driftsys/git-std/commit/34bd5ab
[68e7b33]: https://github.com/driftsys/git-std/commit/68e7b33
[#340]: https://github.com/driftsys/git-std/issues/340
[ee98800]: https://github.com/driftsys/git-std/commit/ee98800
[0a939cf]: https://github.com/driftsys/git-std/commit/0a939cf
[7877bdf]: https://github.com/driftsys/git-std/commit/7877bdf
[0ba7163]: https://github.com/driftsys/git-std/commit/0ba7163
[c677616]: https://github.com/driftsys/git-std/commit/c677616
[d3f3d2e]: https://github.com/driftsys/git-std/commit/d3f3d2e
[#316]: https://github.com/driftsys/git-std/issues/316
[add2ffe]: https://github.com/driftsys/git-std/commit/add2ffe
[#329]: https://github.com/driftsys/git-std/issues/329
[#318]: https://github.com/driftsys/git-std/issues/318
[#317]: https://github.com/driftsys/git-std/issues/317
[3496a5f]: https://github.com/driftsys/git-std/commit/3496a5f
[#315]: https://github.com/driftsys/git-std/issues/315
[6b9d2b7]: https://github.com/driftsys/git-std/commit/6b9d2b7
[526cf92]: https://github.com/driftsys/git-std/commit/526cf92
[e37ccd8]: https://github.com/driftsys/git-std/commit/e37ccd8
[2327d66]: https://github.com/driftsys/git-std/commit/2327d66
[#325]: https://github.com/driftsys/git-std/issues/325
[3cc9bf0]: https://github.com/driftsys/git-std/commit/3cc9bf0
[#324]: https://github.com/driftsys/git-std/issues/324
[9b8b2cb]: https://github.com/driftsys/git-std/commit/9b8b2cb
[52a5785]: https://github.com/driftsys/git-std/commit/52a5785
[63d02f6]: https://github.com/driftsys/git-std/commit/63d02f6
[0fa6925]: https://github.com/driftsys/git-std/commit/0fa6925

## [0.6.0] (2026-03-28)

### Features

- **git-std:** add --format json to bump, hooks list, and hooks run ([#62])
  ([afed2b0]), closes [#62]
- **git-std:** generate man pages and auto-publish to crates.io ([5c897a5]),
  closes [#192], closes #265
- **standard-commit:** add process commit detection and revert type ([c10accb])

### Bug Fixes

- **git-std:** remove duplicate 0.6.0 changelog section ([9ce1cc2])
- **git-std:** align hooks list output with spec ([ad29f1c]), closes [#113]
- **git-std:** harden pre-commit stash dance ([c8a6fcb])
- **git-std:** always allow release scope in lint config ([cb572b5])

### Refactoring

- **git-std:** remove result_line, unify with info ([166f874]), closes [#243]
- **git-std:** use neutral label for stable advance output ([7d2d7a8]), closes
  [#303]

### Documentation

- **git-std:** publish JSON Schema for .git-std.toml ([#61]) ([c9c0307]), closes
  [#61]

[0.6.0]: https://github.com/driftsys/git-std/compare/v0.5.1...v0.6.0
[afed2b0]: https://github.com/driftsys/git-std/commit/afed2b0
[#62]: https://github.com/driftsys/git-std/issues/62
[5c897a5]: https://github.com/driftsys/git-std/commit/5c897a5
[#192]: https://github.com/driftsys/git-std/issues/192
[c10accb]: https://github.com/driftsys/git-std/commit/c10accb
[9ce1cc2]: https://github.com/driftsys/git-std/commit/9ce1cc2
[ad29f1c]: https://github.com/driftsys/git-std/commit/ad29f1c
[#113]: https://github.com/driftsys/git-std/issues/113
[c8a6fcb]: https://github.com/driftsys/git-std/commit/c8a6fcb
[cb572b5]: https://github.com/driftsys/git-std/commit/cb572b5
[166f874]: https://github.com/driftsys/git-std/commit/166f874
[#243]: https://github.com/driftsys/git-std/issues/243
[7d2d7a8]: https://github.com/driftsys/git-std/commit/7d2d7a8
[#303]: https://github.com/driftsys/git-std/issues/303
[c9c0307]: https://github.com/driftsys/git-std/commit/c9c0307
[#61]: https://github.com/driftsys/git-std/issues/61

## [0.5.1] (2026-03-26)

### Bug Fixes

- **git-std:** use human-first messages in bootstrap output ([67386d6]), refs
  [#300]

### Refactoring

- **git-std:** use human-first messages across CLI output ([7f23198]), closes
  [#300]

[0.5.1]: https://github.com/driftsys/git-std/compare/v0.5.0...v0.5.1
[67386d6]: https://github.com/driftsys/git-std/commit/67386d6
[#300]: https://github.com/driftsys/git-std/issues/300
[7f23198]: https://github.com/driftsys/git-std/commit/7f23198

## [0.5.0] (2026-03-26)

### Features

- **git-std:** add bootstrap subcommand for post-clone setup ([8f4af3d]), closes
  [#294], closes #295, closes #296

### Bug Fixes

- **bump:** gate lock file sync on whether corresponding version file was
  updated ([#291]) ([0b78fa5]), closes [#290]
- **bump:** detect workspace Cargo.toml in lock sync via ends_with match
  ([7aac99e]), closes [#289]
- **release:** include Cargo.lock in v0.4.2 release ([a64f245])

[0.5.0]: https://github.com/driftsys/git-std/compare/v0.4.2...v0.5.0
[8f4af3d]: https://github.com/driftsys/git-std/commit/8f4af3d
[#294]: https://github.com/driftsys/git-std/issues/294
[0b78fa5]: https://github.com/driftsys/git-std/commit/0b78fa5
[#291]: https://github.com/driftsys/git-std/issues/291
[#290]: https://github.com/driftsys/git-std/issues/290
[7aac99e]: https://github.com/driftsys/git-std/commit/7aac99e
[#289]: https://github.com/driftsys/git-std/issues/289
[a64f245]: https://github.com/driftsys/git-std/commit/a64f245

## [0.4.2] (2026-03-23)

### Bug Fixes

- **config:** add version_files for workspace crate Cargo.toml files ([041e4a7])

[0.4.2]: https://github.com/driftsys/git-std/compare/v0.4.1...v0.4.2
[041e4a7]: https://github.com/driftsys/git-std/commit/041e4a7

## [0.4.1] (2026-03-23)

### Bug Fixes

- **hooks:** harden stash apply, restage exit check, unreachable, submodule
  guard ([#286]) ([7545fb2]), closes [#251], [#252], [#278], [#283]
- **hooks:** harden restage_deletions and deduplicate staged queries ([#275])
  ([0b86b3e])
- **hooks:** preserve staged deletions in pre-commit fix mode ([#273])
  ([f1a50b5]), closes [#268]
- **install:** handle unbound tmp_dir in cleanup trap ([#272]) ([24a6e62]),
  closes [#267]

[0.4.1]: https://github.com/driftsys/git-std/compare/v0.4.0...v0.4.1
[7545fb2]: https://github.com/driftsys/git-std/commit/7545fb2
[#286]: https://github.com/driftsys/git-std/issues/286
[#251]: https://github.com/driftsys/git-std/issues/251
[#252]: https://github.com/driftsys/git-std/issues/252
[#278]: https://github.com/driftsys/git-std/issues/278
[#283]: https://github.com/driftsys/git-std/issues/283
[0b86b3e]: https://github.com/driftsys/git-std/commit/0b86b3e
[#275]: https://github.com/driftsys/git-std/issues/275
[f1a50b5]: https://github.com/driftsys/git-std/commit/f1a50b5
[#273]: https://github.com/driftsys/git-std/issues/273
[#268]: https://github.com/driftsys/git-std/issues/268
[24a6e62]: https://github.com/driftsys/git-std/commit/24a6e62
[#272]: https://github.com/driftsys/git-std/issues/272
[#267]: https://github.com/driftsys/git-std/issues/267

## [0.4.0] (2026-03-18)

### Refactoring

- **git-std:** split cli/bump.rs into focused modules ([#232]) ([b567790])
- **git-std:** split cli/commit.rs into focused modules ([#229]) ([6f7edbc])
- **git-std:** split config.rs into focused modules ([3310a59])
- **git-std:** split cli/hooks.rs into focused modules ([#228]) ([ed8b4e8])
- **tests:** split large test files by tested concept ([#227]) ([3ee6c84])
- **git-std:** route all eprintln! calls through ui:: helpers ([#226])
  ([8d1d5b2])
- **standard-version:** split calver.rs into calver/mod.rs + parse.rs + bump.rs
  ([#225]) ([fbade38]), closes [#211]
- **standard-changelog:** extract rendering logic into render.rs ([0d2e48f])
- **standard-changelog:** extract link utilities into link.rs ([013820c])
- **standard-changelog:** extract build_release into build.rs ([78c7590])
- **standard-changelog:** extract host detection into host.rs ([796ad3b])
- **standard-changelog:** extract date utilities into date.rs ([7b61363])
- **standard-changelog:** extract model types into model.rs ([6489db7])

### Documentation

- update USAGE.md for release prep ([#255]) ([81f3641])

### Features

- **hooks:** add fix-mode prefix (~) with stash dance for pre-commit ([#197])
  ([db756a9]), closes [#197]
- **config:** add config list/get subcommands ([#237]) ([756f31f])
- **hooks:** add real-time visual feedback during hook execution ([#236])
  ([ba81961])
- **bump:** sync ecosystem lock files after version bump ([#233]) ([f4ab29d])
- **commit:** improve commit command output ([#200]) ([ad0e7d6])
- **standard-githooks:** enable/disable commands, .off shims, and install prompt
  ([f29ff70]), closes [#198]

### Bug Fixes

- **git-std:** replace Box<dyn Error> with anyhow::Result ([#224]) ([fe8d246]),
  closes [#203]
- **spec:** remove dead_code warnings in spec/support/mod.rs ([#223])
  ([1f89b39]), closes [#219]
- **commit:** fail fast with clear error when stdin is not a TTY ([#222])
  ([dc7062b]), closes [#201]
- **standard-version:** replace Box<dyn Error> with typed VersionError ([#221])
  ([5c768ba]), closes [#202]
- **git-std:** update help snapshot to remove self-update entry ([2adfa52])
- **git-std:** remove self-update test references after command removal
  ([7442f77])
- **git-std:** fix install.sh download URL, target names, and tarball extraction
  ([a56b5d6]), closes [#187]
- **standard-commit:** require scope in strict mode when auto-discovery finds
  none ([b2a57c1]), closes [#190]
- **git-std:** remove unimplemented self-update command and spec section
  ([b369b24])

[0.4.0]: https://github.com/driftsys/git-std/compare/v0.3.0...v0.4.0
[b567790]: https://github.com/driftsys/git-std/commit/b567790
[#232]: https://github.com/driftsys/git-std/issues/232
[6f7edbc]: https://github.com/driftsys/git-std/commit/6f7edbc
[#229]: https://github.com/driftsys/git-std/issues/229
[3310a59]: https://github.com/driftsys/git-std/commit/3310a59
[ed8b4e8]: https://github.com/driftsys/git-std/commit/ed8b4e8
[#228]: https://github.com/driftsys/git-std/issues/228
[3ee6c84]: https://github.com/driftsys/git-std/commit/3ee6c84
[#227]: https://github.com/driftsys/git-std/issues/227
[8d1d5b2]: https://github.com/driftsys/git-std/commit/8d1d5b2
[#226]: https://github.com/driftsys/git-std/issues/226
[fbade38]: https://github.com/driftsys/git-std/commit/fbade38
[#225]: https://github.com/driftsys/git-std/issues/225
[#211]: https://github.com/driftsys/git-std/issues/211
[0d2e48f]: https://github.com/driftsys/git-std/commit/0d2e48f
[013820c]: https://github.com/driftsys/git-std/commit/013820c
[78c7590]: https://github.com/driftsys/git-std/commit/78c7590
[796ad3b]: https://github.com/driftsys/git-std/commit/796ad3b
[7b61363]: https://github.com/driftsys/git-std/commit/7b61363
[6489db7]: https://github.com/driftsys/git-std/commit/6489db7
[81f3641]: https://github.com/driftsys/git-std/commit/81f3641
[#255]: https://github.com/driftsys/git-std/issues/255
[db756a9]: https://github.com/driftsys/git-std/commit/db756a9
[#197]: https://github.com/driftsys/git-std/issues/197
[756f31f]: https://github.com/driftsys/git-std/commit/756f31f
[#237]: https://github.com/driftsys/git-std/issues/237
[ba81961]: https://github.com/driftsys/git-std/commit/ba81961
[#236]: https://github.com/driftsys/git-std/issues/236
[f4ab29d]: https://github.com/driftsys/git-std/commit/f4ab29d
[#233]: https://github.com/driftsys/git-std/issues/233
[ad0e7d6]: https://github.com/driftsys/git-std/commit/ad0e7d6
[#200]: https://github.com/driftsys/git-std/issues/200
[f29ff70]: https://github.com/driftsys/git-std/commit/f29ff70
[#198]: https://github.com/driftsys/git-std/issues/198
[fe8d246]: https://github.com/driftsys/git-std/commit/fe8d246
[#224]: https://github.com/driftsys/git-std/issues/224
[#203]: https://github.com/driftsys/git-std/issues/203
[1f89b39]: https://github.com/driftsys/git-std/commit/1f89b39
[#223]: https://github.com/driftsys/git-std/issues/223
[#219]: https://github.com/driftsys/git-std/issues/219
[dc7062b]: https://github.com/driftsys/git-std/commit/dc7062b
[#222]: https://github.com/driftsys/git-std/issues/222
[#201]: https://github.com/driftsys/git-std/issues/201
[5c768ba]: https://github.com/driftsys/git-std/commit/5c768ba
[#221]: https://github.com/driftsys/git-std/issues/221
[#202]: https://github.com/driftsys/git-std/issues/202
[2adfa52]: https://github.com/driftsys/git-std/commit/2adfa52
[7442f77]: https://github.com/driftsys/git-std/commit/7442f77
[a56b5d6]: https://github.com/driftsys/git-std/commit/a56b5d6
[#187]: https://github.com/driftsys/git-std/issues/187
[b2a57c1]: https://github.com/driftsys/git-std/commit/b2a57c1
[#190]: https://github.com/driftsys/git-std/issues/190
[b369b24]: https://github.com/driftsys/git-std/commit/b369b24

## [0.3.0] (2026-03-17)

### Bug Fixes

- **ci:** replace deprecated macos-13 with macos-latest in release workflow
  ([3402c6c])
- **docs:** remove stray character in SPEC.md table ([1bde589])
- **ci:** pin cross to v0.2.5 for aarch64-musl builds ([792c7e8])
- **spec:** add missing snapshot files for patch and stable bump tests
  ([df36f45])
- improve error messages and diagnostics across CLI ([1337200]), closes [#67]
- **spec:** simplify changelog range snapshot for parallel test stability
  ([6ea2dac])
- **spec:** use flexible matching for changelog range snapshot ([239d000])
- **spec:** correct changelog range snapshot section order ([d8c8ad7])
- **ci:** use cargo install for mdbook instead of broken URL ([3f79092])

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

### Documentation

- sync SPEC, USAGE, and README with current CLI ([3d0f36a])
- defer org-wide policies to driftsys/.github ([f498fca])
- add AI policy section to CONTRIBUTING.md ([eec0a62])
- replace docs.rs badge with user guide badge for binary crate ([a666bbc])
- add badges, book links, and documentation metadata ([6696f5d])
- move issue model to CONTRIBUTING.md, trim AGENTS.md ([cae0d4b])

[0.3.0]: https://github.com/driftsys/git-std/compare/v0.2.0...v0.3.0
[3402c6c]: https://github.com/driftsys/git-std/commit/3402c6c
[1bde589]: https://github.com/driftsys/git-std/commit/1bde589
[792c7e8]: https://github.com/driftsys/git-std/commit/792c7e8
[df36f45]: https://github.com/driftsys/git-std/commit/df36f45
[1337200]: https://github.com/driftsys/git-std/commit/1337200
[#67]: https://github.com/driftsys/git-std/issues/67
[6ea2dac]: https://github.com/driftsys/git-std/commit/6ea2dac
[239d000]: https://github.com/driftsys/git-std/commit/239d000
[d8c8ad7]: https://github.com/driftsys/git-std/commit/d8c8ad7
[3f79092]: https://github.com/driftsys/git-std/commit/3f79092
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
[3d0f36a]: https://github.com/driftsys/git-std/commit/3d0f36a
[f498fca]: https://github.com/driftsys/git-std/commit/f498fca
[eec0a62]: https://github.com/driftsys/git-std/commit/eec0a62
[a666bbc]: https://github.com/driftsys/git-std/commit/a666bbc
[6696f5d]: https://github.com/driftsys/git-std/commit/6696f5d
[cae0d4b]: https://github.com/driftsys/git-std/commit/cae0d4b

## [0.2.0] (2026-03-16)

### Bug Fixes

- **bump:** warn on system clock failure in calver date ([eb5fcee]), closes
  [#131]
- **bump:** address calver review findings ([b5c87bc])
- **bump:** address review findings from multi-ecosystem version files ([#122])
  ([e670128])
- **bump:** sync Cargo.lock after version update ([ee60541])

### Refactoring

- **hooks:** improve API consistency and extract helpers ([#116]) ([a14a669]),
  closes [#98]

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

[0.2.0]: https://github.com/driftsys/git-std/compare/v0.1.0...v0.2.0
[eb5fcee]: https://github.com/driftsys/git-std/commit/eb5fcee
[#131]: https://github.com/driftsys/git-std/issues/131
[b5c87bc]: https://github.com/driftsys/git-std/commit/b5c87bc
[e670128]: https://github.com/driftsys/git-std/commit/e670128
[#122]: https://github.com/driftsys/git-std/issues/122
[ee60541]: https://github.com/driftsys/git-std/commit/ee60541
[a14a669]: https://github.com/driftsys/git-std/commit/a14a669
[#116]: https://github.com/driftsys/git-std/issues/116
[#98]: https://github.com/driftsys/git-std/issues/98
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
