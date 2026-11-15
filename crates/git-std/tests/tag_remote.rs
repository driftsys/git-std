mod tag_support;
use tag_support::*;

fn remote(root: &std::path::Path) -> tempfile::TempDir {
    let r = tempfile::tempdir().unwrap();
    git(r.path(), &["init", "--bare"]);
    git(
        root,
        &["remote", "add", "publication", r.path().to_str().unwrap()],
    );
    git(root, &["push", "publication", "main"]);
    r
}
#[test]
fn remote_branch_is_authoritative_and_only_selected_tags_are_pushed() {
    let r = repo("", "1.2.3", "chore(release): 1.2.3");
    let bare = remote(r.path());
    let oid = git(r.path(), &["rev-parse", "HEAD"]);
    git(r.path(), &["checkout", "--detach"]);
    git(r.path(), &["branch", "-D", "main"]);
    git(r.path(), &["tag", "-a", "unrelated", "-m", "unrelated"]);
    git(r.path(), &["config", "push.followTags", "true"]);
    let refs = git(r.path(), &["show-ref"]);
    ok(tag(r.path(), &[&oid, "--push", "publication", "--dry-run"]));
    assert_eq!(git(r.path(), &["show-ref"]), refs);
    ok(tag(r.path(), &[&oid, "--push", "publication"]));
    assert_eq!(git(bare.path(), &["tag", "-l"]), "v1.2.3");
    assert_eq!(git(bare.path(), &["rev-parse", "v1.2.3^{commit}"]), oid);
    ok(tag(r.path(), &[&oid, "--push", "publication"]));
}
#[test]
fn unmerged_target_and_distinct_push_endpoint_refuse_before_creation() {
    let r = repo("", "1.2.2", "chore: initial");
    let _bare = remote(r.path());
    std::fs::write(r.path().join("VERSION"), "1.2.3").unwrap();
    git(r.path(), &["add", "."]);
    git(r.path(), &["commit", "-m", "chore(release): 1.2.3"]);
    assert_eq!(
        tag(r.path(), &["HEAD", "--push", "publication"])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(git(r.path(), &["tag", "-l"]), "");
    git(r.path(), &["push", "publication", "main"]);
    git(
        r.path(),
        &["config", "remote.publication.pushurl", "/different"],
    );
    assert_eq!(
        tag(r.path(), &["HEAD", "--push", "publication"])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(git(r.path(), &["tag", "-l"]), "");
}
#[test]
fn remote_only_annotated_object_is_adopted_exactly() {
    let r = repo("", "1.2.3", "chore(release): 1.2.3");
    let bare = remote(r.path());
    git(
        bare.path(),
        &[
            "-c",
            "user.name=Remote",
            "-c",
            "user.email=remote@example.com",
            "tag",
            "-a",
            "v1.2.3",
            "main",
            "-m",
            "1.2.3",
        ],
    );
    let oid = git(bare.path(), &["rev-parse", "v1.2.3"]);
    ok(tag(r.path(), &["HEAD", "--push", "publication"]));
    assert_eq!(git(r.path(), &["rev-parse", "v1.2.3"]), oid);
}
#[test]
fn atomic_support_is_required_without_fallback() {
    let r = repo("", "1.2.3", "chore(release): 1.2.3");
    let bare = remote(r.path());
    git(bare.path(), &["config", "receive.advertiseAtomic", "false"]);
    let o = tag(r.path(), &["HEAD", "--push", "publication"]);
    assert_eq!(o.status.code(), Some(2), "{o:?}");
    assert_eq!(git(bare.path(), &["tag", "-l"]), "");
    assert_eq!(git(r.path(), &["tag", "-l"]), "v1.2.3");
}
