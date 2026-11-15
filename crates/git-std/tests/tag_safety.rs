mod tag_support;
use tag_support::*;

#[test]
fn incomplete_history_refuses_without_tags() {
    for promisor in [false, true] {
        let r = repo("", "1.2.3", "chore(release): 1.2.3");
        if promisor {
            git(r.path(), &["config", "remote.origin.promisor", "true"]);
        } else {
            std::fs::write(
                r.path().join(".git/shallow"),
                format!("{}\n", git(r.path(), &["rev-parse", "HEAD"])),
            )
            .unwrap();
        }
        let o = tag(r.path(), &["HEAD"]);
        assert_eq!(o.status.code(), Some(1), "{o:?}");
        assert!(git(r.path(), &["tag", "-l"]).is_empty());
    }
}

#[test]
fn lfs_pointer_is_not_a_release_version() {
    let r = repo(
        "",
        "version https://git-lfs.github.com/spec/v1\noid sha256:0000\nsize 5\n",
        "chore(release): 1.2.3",
    );
    let o = tag(r.path(), &["HEAD"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("LFS pointer"));
}

#[test]
fn inherited_cargo_versions_are_validated_from_target() {
    let r = repo("", "1.2.3", "chore: initial");
    std::fs::write(
        r.path().join("Cargo.toml"),
        "[workspace]\nmembers=['core']\n[workspace.package]\nversion='1.2.3'\n",
    )
    .unwrap();
    std::fs::create_dir(r.path().join("core")).unwrap();
    std::fs::write(
        r.path().join("core/Cargo.toml"),
        "[package]\nname='core'\nversion.workspace=true\n",
    )
    .unwrap();
    git(r.path(), &["add", "."]);
    git(r.path(), &["commit", "-m", "chore(release): 1.2.3"]);
    ok(tag(r.path(), &["HEAD", "--dry-run"]));
    std::fs::write(
        r.path().join("core/Cargo.toml"),
        "[package]\nname='core'\nversion='9.0.0'\n",
    )
    .unwrap();
    git(r.path(), &["add", "."]);
    git(r.path(), &["commit", "-m", "chore(release): 1.2.3"]);
    assert_eq!(tag(r.path(), &["HEAD"]).status.code(), Some(1));
    assert!(git(r.path(), &["tag", "-l"]).is_empty());
}

#[test]
fn colliding_package_templates_refuse_before_any_creation() {
    let r = repo(
        "monorepo=true\n[versioning]\ntag_template='v{version}'\n[[packages]]\nname='core'\npath='core'",
        "1.2.3",
        "chore: initial",
    );
    std::fs::create_dir(r.path().join("core")).unwrap();
    std::fs::write(r.path().join("core/VERSION"), "1.2.3").unwrap();
    git(r.path(), &["add", "."]);
    git(
        r.path(),
        &["commit", "-m", "chore(release): v1.2.3, core@1.2.3"],
    );
    let o = tag(r.path(), &["HEAD"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("collide"));
    assert!(git(r.path(), &["tag", "-l"]).is_empty());
}
