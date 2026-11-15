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
