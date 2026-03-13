use std::process::Command;

#[test]
fn cargo_build_succeeds() {
    let status = Command::new("cargo")
        .args(["build", "--package", "git-std"])
        .status()
        .expect("failed to run cargo build");
    assert!(status.success(), "cargo build failed");
}

#[test]
fn binary_name_is_git_std() {
    let output = Command::new("cargo")
        .args(["build", "--package", "git-std", "--message-format=json"])
        .output()
        .expect("failed to run cargo build");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"executable\"") && stdout.contains("git-std"),
        "binary should be named git-std"
    );
}
