mod tag_support;
use tag_support::*;

fn manifest_repo(name: &str, content: &str) -> tempfile::TempDir {
    let r = repo("", "1.2.3", "chore: initial");
    git(r.path(), &["rm", "VERSION"]);
    std::fs::write(r.path().join(name), content).unwrap();
    git(r.path(), &["add", "."]);
    git(r.path(), &["commit", "-m", "chore(release): 1.2.3"]);
    r
}

#[test]
fn jsonc_comments_cannot_override_structural_version_authority() {
    for name in ["deno.json", "deno.jsonc", "project.json"] {
        for (content, basis) in [
            (r#"{/* "version":"0.0.1" */ "version":"1.2.3"}"#, "manifest"),
            (
                r#"{/* "version":"0.0.1" */ "name":"example"}"#,
                "release_subject",
            ),
            (
                r#"{"nested":{"version":"0.0.1"}, "name":"example"}"#,
                "release_subject",
            ),
        ] {
            let r = manifest_repo(name, content);
            let value = ok(tag(r.path(), &["HEAD", "--format", "json"]));
            assert_eq!(
                value["tags"][0]["version_basis"], basis,
                "{name}: {content}"
            );
            assert_eq!(
                git(r.path(), &["rev-parse", "v1.2.3^{}"]),
                git(r.path(), &["rev-parse", "HEAD"])
            );
        }
    }
}

#[test]
fn mismatched_structural_manifest_versions_refuse_without_ref_changes() {
    for (name, content) in [
        ("package.json", r#"{"version":"9.9.9"}"#),
        ("project.json", r#"{/* comment */ "version":"9.9.9"}"#),
        ("deno.json", r#"{/* comment */ "version":"9.9.9"}"#),
        ("deno.jsonc", r#"{/* comment */ "version":"9.9.9"}"#),
        ("project.yaml", "version: '9.9.9'\n"),
        ("project.yml", "version: '9.9.9'\n"),
        ("pubspec.yaml", "version: '9.9.9+7'\n"),
    ] {
        let r = manifest_repo(name, content);
        let before = git(r.path(), &["show-ref"]);
        let out = tag(r.path(), &["HEAD"]);
        assert_eq!(out.status.code(), Some(1), "{name}: {out:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("version mismatch"),
            "{name}: {out:?}"
        );
        assert_eq!(git(r.path(), &["show-ref"]), before);
    }
}

#[test]
fn strict_json_and_yaml_mapping_requirements_are_enforced() {
    for (name, content) in [
        ("package.json", r#"{/* comment */ "version":"1.2.3"}"#),
        ("package.json", r#"{"version":"1.2.3",}"#),
        ("project.yaml", "- version: 1.2.3\n"),
        ("pubspec.yaml", "1.2.3\n"),
    ] {
        let r = manifest_repo(name, content);
        let before = git(r.path(), &["show-ref"]);
        assert_eq!(tag(r.path(), &["HEAD"]).status.code(), Some(1));
        assert_eq!(git(r.path(), &["show-ref"]), before);
    }
}
