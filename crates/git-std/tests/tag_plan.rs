mod tag_support;
use tag_support::*;
#[test]
fn historical_configuration_and_dirty_checkout_are_isolated() {
    let r = repo(
        "[versioning]\ntag_prefix='release-'",
        "1.2.3",
        "chore(release): 1.2.3 (#12)",
    );
    let oid = git(r.path(), &["rev-parse", "HEAD"]);
    std::fs::write(r.path().join("VERSION"), "9.0.0").unwrap();
    git(r.path(), &["add", "VERSION"]);
    let before = git(r.path(), &["write-tree"]);
    let v = ok(tag(r.path(), &[&oid, "--dry-run", "--format", "json"]));
    assert_eq!(v["tags"][0]["name"], "release-1.2.3");
    assert_eq!(git(r.path(), &["write-tree"]), before);
    assert_eq!(
        std::fs::read_to_string(r.path().join("VERSION")).unwrap(),
        "9.0.0"
    );
}
#[test]
fn bad_identity_versions_configuration_and_sources_refuse_without_refs() {
    for (config, version, subject) in [
        ("", "1.2.4", "chore(release): 1.2.3"),
        ("", "1.2.3", "fix: release"),
        ("monorepo='yes'", "1.2.3", "chore(release): 1.2.3"),
        (
            "[[version_files]]\npath='../outside'\nregex='(.*)'",
            "1.2.3",
            "chore(release): 1.2.3",
        ),
        (
            "[[version_files]]\npath='missing'\nregex='(.*)'",
            "1.2.3",
            "chore(release): 1.2.3",
        ),
    ] {
        let r = repo(config, version, subject);
        let o = tag(r.path(), &["HEAD", "--format", "json"]);
        assert_eq!(o.status.code(), Some(1), "{o:?}");
        assert_eq!(git(r.path(), &["tag", "-l"]), "");
    }
}
#[test]
fn monorepo_selects_only_released_scoped_package() {
    let r = repo(
        "monorepo=true\n[[packages]]\nname='@scope/core'\npath='core'\n[[packages]]\nname='api'\npath='api'",
        "2.0.0",
        "chore: initial",
    );
    for (p, v) in [("core", "1.2.3"), ("api", "4.0.0")] {
        std::fs::create_dir(r.path().join(p)).unwrap();
        std::fs::write(r.path().join(p).join("VERSION"), v).unwrap();
    }
    git(r.path(), &["add", "."]);
    git(
        r.path(),
        &[
            "commit",
            "-m",
            "chore(release): v2.0.0, @scope/core@1.2.3, api@4.0.0",
        ],
    );
    ok(tag(r.path(), &["HEAD", "--package", "@scope/core"]));
    assert_eq!(git(r.path(), &["tag", "-l"]), "@scope/core@1.2.3");
}
#[test]
fn explicit_branch_and_manifestless_release() {
    let r = repo("", "1.2.3", "chore: initial");
    git(r.path(), &["switch", "-c", "release/1"]);
    git(r.path(), &["rm", "VERSION"]);
    git(r.path(), &["commit", "-m", "chore(release): 1.2.3"]);
    assert_eq!(tag(r.path(), &["HEAD"]).status.code(), Some(1));
    ok(tag(r.path(), &["HEAD", "--branch", "release/1"]));
}
#[cfg(unix)]
#[test]
fn symlink_source_is_refused() {
    let r = repo("", "1.2.3", "chore: initial");
    std::fs::remove_file(r.path().join("VERSION")).unwrap();
    std::os::unix::fs::symlink("/etc/passwd", r.path().join("VERSION")).unwrap();
    git(r.path(), &["add", "."]);
    git(r.path(), &["commit", "-m", "chore(release): 1.2.3"]);
    assert_eq!(tag(r.path(), &["HEAD"]).status.code(), Some(1));
}

#[test]
fn workspace_discovery_cannot_escape_the_snapshot() {
    let r = repo("monorepo=true", "1.2.3", "chore: initial");
    std::fs::write(
        r.path().join("Cargo.toml"),
        "[workspace]\nmembers=['../outside/*']\n",
    )
    .unwrap();
    git(r.path(), &["add", "."]);
    git(r.path(), &["commit", "-m", "chore(release): v1.2.3"]);
    let o = tag(r.path(), &["HEAD"]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("unsafe release source path"),
        "{o:?}"
    );
}

#[test]
fn configured_custom_file_must_match_its_own_regex() {
    let r = repo(
        "[[version_files]]\npath='VERSION'\nregex='not-present=(.*)'",
        "1.2.3",
        "chore(release): 1.2.3",
    );
    let o = tag(r.path(), &["HEAD"]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
}

#[test]
fn calver_custom_pattern_and_tag_collisions() {
    let r = repo(
        "scheme='calver'\n[versioning]\ntag_prefix='date-'\ncalver_format='YYYY.0M.PATCH'",
        "2026.03.12",
        "chore(release): 2026.03.12",
    );
    ok(tag(r.path(), &["HEAD"]));
    assert_eq!(git(r.path(), &["tag", "-l"]), "date-2026.03.12");
}

#[test]
fn structural_manifest_versions_cannot_be_mistaken_for_manifestless() {
    for (name, content) in [
        (
            "Cargo.toml",
            "[package] # header comment\nname='p'\nversion='9.9.9'\n",
        ),
        ("Cargo.toml", "['package']\nname='p'\nversion='9.9.9'\n"),
        ("package.json", "{\"name\":\"p\",\"version\":123}"),
    ] {
        let r = repo("", "1.2.3", "chore: initial");
        git(r.path(), &["rm", "VERSION"]);
        std::fs::write(r.path().join(name), content).unwrap();
        git(r.path(), &["add", "."]);
        git(r.path(), &["commit", "-m", "chore(release): 1.2.3"]);
        let out = tag(r.path(), &["HEAD"]);
        assert_eq!(out.status.code(), Some(1), "{name}: {out:?}");
        assert!(git(r.path(), &["tag", "-l"]).is_empty());
    }
}

#[test]
fn quoted_commented_cargo_version_is_manifest_authority() {
    let r = repo("", "1.2.3", "chore: initial");
    git(r.path(), &["rm", "VERSION"]);
    std::fs::write(
        r.path().join("Cargo.toml"),
        "['package'] # valid header\nname='p'\nversion='1.2.3'\n",
    )
    .unwrap();
    git(r.path(), &["add", "."]);
    git(r.path(), &["commit", "-m", "chore(release): 1.2.3"]);
    let value = ok(tag(r.path(), &["HEAD", "--dry-run", "--format", "json"]));
    assert_eq!(value["tags"][0]["version_basis"], "manifest");
}
