mod tag_support;
use tag_support::*;

fn workspace(config: &str, member: &str) -> tempfile::TempDir {
    let r = repo(config, "1.2.3", "chore: initial");
    std::fs::write(
        r.path().join("Cargo.toml"),
        "[workspace]\nmembers=['member']\n[workspace.package]\nversion='1.2.3'\n",
    )
    .unwrap();
    std::fs::create_dir(r.path().join("member")).unwrap();
    std::fs::write(
        r.path().join("member/Cargo.toml"),
        format!("[package]\nname='member'\n{member}\n"),
    )
    .unwrap();
    git(r.path(), &["add", "."]);
    git(r.path(), &["commit", "-m", "chore(release): 1.2.3"]);
    r
}

#[test]
fn independent_unpublished_version_is_not_root_release_authority() {
    let r = workspace("", "publish=false\nversion='0.3.0'");
    ok(tag(r.path(), &["HEAD"]));
    assert_eq!(
        git(r.path(), &["rev-parse", "v1.2.3^{}"]),
        git(r.path(), &["rev-parse", "HEAD"])
    );
}

#[test]
fn released_and_malformed_members_still_refuse_without_refs() {
    for member in [
        "version='0.3.0'",
        "publish=true\nversion='0.3.0'",
        "publish=[]\nversion='0.3.0'",
        "publish=false\nversion=123",
        "publish=false\nversion='banana'",
        "publish=false\nversion='0.3.0'\ninvalid=[",
    ] {
        let r = workspace("", member);
        let before = git(r.path(), &["show-ref"]);
        let out = tag(r.path(), &["HEAD"]);
        assert_eq!(out.status.code(), Some(1), "{member}: {out:?}");
        assert_eq!(git(r.path(), &["show-ref"]), before);
    }
}

#[test]
fn explicit_custom_capture_remains_root_release_authority() {
    let r = workspace(
        "[[version_files]]\npath='member/Cargo.toml'\nregex=\"version='([^']+)'\"",
        "publish=false\nversion='0.3.0'",
    );
    let before = git(r.path(), &["show-ref"]);
    assert_eq!(tag(r.path(), &["HEAD"]).status.code(), Some(1));
    assert_eq!(git(r.path(), &["show-ref"]), before);
}

#[test]
fn explicit_monorepo_entry_must_match_even_when_unpublished() {
    let r = workspace(
        "monorepo=true\n[[packages]]\nname='member'\npath='member'",
        "publish=false\nversion='0.3.0'",
    );
    git(
        r.path(),
        &[
            "commit",
            "--allow-empty",
            "-m",
            "chore(release): v1.2.3, member@1.2.3",
        ],
    );
    let before = git(r.path(), &["show-ref"]);
    assert_eq!(
        tag(r.path(), &["HEAD", "--package", "member"])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(git(r.path(), &["show-ref"]), before);
}

#[test]
fn unpublished_members_still_validate_their_other_sources() {
    for member in [
        "publish=false\nversion.workspace=true",
        "publish=false\nversion='0.2.0'",
    ] {
        let r = workspace("", member);
        std::fs::write(r.path().join("member/VERSION"), "0.3.0").unwrap();
        git(r.path(), &["add", "."]);
        git(r.path(), &["commit", "--amend", "--no-edit"]);
        let before = git(r.path(), &["show-ref"]);
        assert_eq!(tag(r.path(), &["HEAD"]).status.code(), Some(1));
        assert_eq!(git(r.path(), &["show-ref"]), before);
    }
}
