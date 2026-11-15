mod tag_support;
use tag_support::*;
#[test]
fn retries_preserve_annotated_object() {
    let r = repo("", "1.2.3", "chore(release): 1.2.3");
    ok(tag(r.path(), &["HEAD"]));
    let oid = git(r.path(), &["rev-parse", "v1.2.3"]);
    let v = ok(tag(r.path(), &["HEAD", "--format", "json"]));
    assert_eq!(v["tags"][0]["local"], "existing");
    assert_eq!(git(r.path(), &["rev-parse", "v1.2.3"]), oid);
}
#[test]
fn lightweight_and_annotation_conflicts_are_not_replaced() {
    for args in [
        vec!["tag", "v1.2.3"],
        vec!["tag", "-a", "v1.2.3", "-m", "wrong"],
    ] {
        let r = repo("", "1.2.3", "chore(release): 1.2.3");
        git(r.path(), &args);
        let oid = git(r.path(), &["rev-parse", "v1.2.3"]);
        assert_eq!(tag(r.path(), &["HEAD"]).status.code(), Some(1));
        assert_eq!(git(r.path(), &["rev-parse", "v1.2.3"]), oid);
    }
}
#[test]
fn unsigned_reuse_cannot_satisfy_sign_request() {
    let r = repo("", "1.2.3", "chore(release): 1.2.3");
    ok(tag(r.path(), &["HEAD"]));
    assert_eq!(tag(r.path(), &["HEAD", "--sign"]).status.code(), Some(1));
}

#[test]
fn matching_annotation_on_wrong_target_cannot_be_reused_or_published() {
    let r = repo("", "1.2.3", "chore(release): 1.2.3");
    git(r.path(), &["tag", "-a", "v1.2.3", "-m", "1.2.3"]);
    let oid = git(r.path(), &["rev-parse", "v1.2.3"]);
    git(
        r.path(),
        &["commit", "--allow-empty", "-m", "chore(release): 1.2.3"],
    );
    let remote = tempfile::tempdir().unwrap();
    git(remote.path(), &["init", "--bare"]);
    git(
        r.path(),
        &["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    git(r.path(), &["push", "origin", "main"]);
    let refs = git(r.path(), &["ls-remote", "origin"]);
    for args in [vec!["HEAD"], vec!["HEAD", "--push", "origin"]] {
        let out = tag(r.path(), &args);
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("tag target conflict"),
            "{out:?}"
        );
        assert_eq!(git(r.path(), &["rev-parse", "v1.2.3"]), oid);
        assert_eq!(git(r.path(), &["ls-remote", "origin"]), refs);
    }
}
