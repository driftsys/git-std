mod tag_support;
use tag_support::*;

#[test]
fn expanded_custom_sources_cannot_escape_snapshot() {
    for path in ["{../outside,VERSION}", "{VERSION,../outside}"] {
        for package in [false, true] {
            let config = if package {
                format!(
                    "monorepo=true\n[[packages]]\nname='core'\npath='core'\n[[packages.version_files]]\npath='{path}'\nregex='(.*)'"
                )
            } else {
                format!("[[version_files]]\npath='{path}'\nregex='(.*)'")
            };
            let r = repo(&config, "1.2.3", "chore(release): 1.2.3");
            let scratch = tempfile::tempdir().unwrap();
            std::fs::write(scratch.path().join("outside"), "private-sentinel").unwrap();
            let before = git(r.path(), &["show-ref"]);
            let out = std::process::Command::new(env!("CARGO_BIN_EXE_git-std"))
                .current_dir(r.path())
                .env("TMPDIR", scratch.path())
                .args(["tag", "HEAD"])
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(1), "{out:?}");
            let diagnostic = String::from_utf8_lossy(&out.stderr);
            assert!(diagnostic.contains("unsafe release source path"), "{out:?}");
            assert!(!diagnostic.contains("private-sentinel"));
            assert_eq!(git(r.path(), &["show-ref"]), before);
        }
    }
}

#[test]
fn root_and_package_changelog_sections_are_supported() {
    let r = repo(
        "monorepo=true\n[changelog]\nhidden=['chore']\n[changelog.sections]\nfeat='Features'\n[[packages]]\nname='core'\npath='core'\n[packages.changelog.sections]\nfix='Repairs'",
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
    ok(tag(r.path(), &["HEAD"]));
    assert_eq!(git(r.path(), &["tag", "-l"]), "core@1.2.3\nv1.2.3");
}

fn manifest_repo(name: &str, content: &str) -> tempfile::TempDir {
    let r = repo("", "1.2.3", "chore: initial");
    git(r.path(), &["rm", "VERSION"]);
    std::fs::write(r.path().join(name), content).unwrap();
    git(r.path(), &["add", "."]);
    git(r.path(), &["commit", "-m", "chore(release): 1.2.3"]);
    r
}

#[test]
fn malformed_or_wrongly_typed_jsonc_and_yaml_refuse() {
    for (name, content) in [
        ("deno.jsonc", "{\"version\":123}"),
        ("deno.jsonc", "{\"version\":\"1.2.3\", broken}"),
        ("pubspec.yaml", "version: 1.2.3\ninvalid: [\n"),
        ("pubspec.yaml", "version: 123\n"),
        ("pubspec.yaml", "version: 1.2.3\n---\nversion: 9.9.9\n"),
        ("project.yaml", "version: [1, 2, 3]\n"),
        ("project.yml", "version: 1.2.3\ninvalid: [\n"),
    ] {
        let r = manifest_repo(name, content);
        let out = tag(r.path(), &["HEAD"]);
        assert_eq!(out.status.code(), Some(1), "{name}: {out:?}");
        assert!(git(r.path(), &["tag", "-l"]).is_empty());
    }
}

#[test]
fn supported_jsonc_comments_and_yaml_versions_are_validated() {
    for (name, content) in [
        (
            "project.json",
            "{ // release\n\"version\": \"1.2.3\" /* note */ }",
        ),
        ("deno.json", "{ // release\n\"version\": \"1.2.3\", }"),
        ("deno.jsonc", "{/* note */ \"version\": \"1.2.3\"}"),
        ("pubspec.yaml", "name: example\nversion: '1.2.3+7'\n"),
        ("project.yaml", "version: \"1.2.3\"\n"),
        ("project.yml", "version: 1.2.3\n"),
    ] {
        let r = manifest_repo(name, content);
        let value = ok(tag(r.path(), &["HEAD", "--format", "json"]));
        assert_eq!(value["tags"][0]["version_basis"], "manifest", "{name}");
        assert_eq!(
            git(r.path(), &["rev-parse", "v1.2.3^{commit}"]),
            git(r.path(), &["rev-parse", "HEAD"])
        );
    }
}

#[test]
fn invalid_changelog_sections_and_hidden_types_are_refused() {
    for config in [
        "[changelog]\nsections=['feat']",
        "[changelog.sections]\nfeat=123",
        "[changelog]\nhidden='chore'",
        "[[packages]]\nname='core'\npath='core'\n[packages.changelog.sections]\nfix=false",
    ] {
        let r = repo(config, "1.2.3", "chore(release): 1.2.3");
        assert_eq!(tag(r.path(), &["HEAD"]).status.code(), Some(1));
        assert!(git(r.path(), &["tag", "-l"]).is_empty());
    }
}

#[test]
fn versionless_jsonc_and_yaml_can_use_release_subject() {
    for (name, content) in [
        ("deno.jsonc", "{/* versionless */ \"name\":\"example\"}"),
        ("pubspec.yaml", "name: example\n"),
        ("project.yaml", "name: example\n"),
    ] {
        let r = manifest_repo(name, content);
        let value = ok(tag(r.path(), &["HEAD", "--format", "json"]));
        assert_eq!(value["tags"][0]["version_basis"], "release_subject");
    }
}
