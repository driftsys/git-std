use std::path::{Path, PathBuf};

/// Fluent builder for test git repositories.
///
/// Creates a temporary directory with a git repository, configures
/// user identity, and provides methods to set up hooks files,
/// config, commits, and tags.
pub struct TestRepo {
    dir: tempfile::TempDir,
    file_counter: usize,
}

impl TestRepo {
    /// Create a new test repository with `git init` and user config.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        git(dir.path(), &["init"]);
        git(dir.path(), &["config", "user.name", "Test"]);
        git(dir.path(), &["config", "user.email", "test@test.com"]);

        Self {
            dir,
            file_counter: 0,
        }
    }

    /// Write a `.githooks/<name>.hooks` file with the given content.
    pub fn with_hooks_file(self, name: &str, content: &str) -> Self {
        let hooks_dir = self.dir.path().join(".githooks");
        std::fs::create_dir_all(&hooks_dir).expect("failed to create .githooks dir");
        std::fs::write(hooks_dir.join(format!("{name}.hooks")), content)
            .expect("failed to write hooks file");
        self
    }

    /// Write a `.git-std.toml` config file.
    pub fn with_config(self, content: &str) -> Self {
        std::fs::write(self.dir.path().join(".git-std.toml"), content)
            .expect("failed to write config");
        self
    }

    /// Write a minimal `Cargo.toml` with the given version.
    pub fn with_cargo_toml(self, version: &str) -> Self {
        std::fs::write(
            self.dir.path().join("Cargo.toml"),
            format!(
                "[package]\nname = \"test-pkg\"\nversion = \"{version}\"\nedition = \"2021\"\n"
            ),
        )
        .expect("failed to write Cargo.toml");
        self
    }

    /// Write a minimal `package.json` with the given version.
    pub fn with_package_json(self, version: &str) -> Self {
        std::fs::write(
            self.dir.path().join("package.json"),
            format!("{{\n  \"name\": \"test-pkg\",\n  \"version\": \"{version}\"\n}}\n"),
        )
        .expect("failed to write package.json");
        self
    }

    /// Create a file, stage it, and commit with the given message.
    pub fn add_commit(&mut self, message: &str) -> &mut Self {
        self.file_counter += 1;
        let filename = format!("file-{}.txt", self.file_counter);
        std::fs::write(self.dir.path().join(&filename), message)
            .expect("failed to write commit file");

        git(self.dir.path(), &["add", &filename]);
        git(self.dir.path(), &["commit", "-m", message]);

        self
    }

    /// Create an annotated tag at HEAD.
    pub fn create_tag(&self, name: &str) -> &Self {
        git(self.dir.path(), &["tag", "-a", name, "-m", name]);
        self
    }

    /// Return the path to the temporary directory.
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Return the path to the `git-std` binary built by cargo.
    pub fn bin_path() -> PathBuf {
        assert_cmd::cargo::cargo_bin("git-std")
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("failed to run git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
