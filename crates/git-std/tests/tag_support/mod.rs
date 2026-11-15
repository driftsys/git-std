use std::{
    path::Path,
    process::{Command, Output},
};
pub fn git(root: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}: {o:?}");
    String::from_utf8(o.stdout).unwrap().trim_end().into()
}
pub fn repo(config: &str, version: &str, subject: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    git(root.path(), &["init", "-b", "main"]);
    git(root.path(), &["config", "user.name", "Test"]);
    git(root.path(), &["config", "user.email", "test@example.com"]);
    std::fs::write(root.path().join(".git-std.toml"), config).unwrap();
    std::fs::write(root.path().join("VERSION"), version).unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", subject]);
    root
}
pub fn tag(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_git-std"))
        .current_dir(root)
        .arg("tag")
        .args(args)
        .output()
        .unwrap()
}
pub fn ok(o: Output) -> serde_json::Value {
    assert!(o.status.success(), "{o:?}");
    if o.stdout.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&o.stdout).unwrap()
    }
}
