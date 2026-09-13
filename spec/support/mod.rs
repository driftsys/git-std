use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Observable repository state used to prove a command was non-mutating.
#[derive(Debug, PartialEq, Eq)]
pub struct RepoState {
    head: String,
    tags: String,
    status: String,
    files: BTreeMap<PathBuf, Vec<u8>>,
}

/// Fluent builder for test git repositories.
///
/// Creates a temporary directory with a git repository, configures
/// user identity, and provides methods to set up hooks files,
/// config, commits, and tags.
pub struct TestRepo {
    dir: tempfile::TempDir,
    // Incremented by add_commit — used only in test binaries that call it.
    #[allow(dead_code)]
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

    /// Create `.githooks/`, set `core.hooksPath = .githooks`, create `.git-std.toml`,
    /// and set up `.git-blame-ignore-revs` with `blame.ignoreRevsFile` configured.
    /// Required for `doctor` to exit 0 (all health checks satisfied).
    // Used by: spec/tests/doctor.rs
    #[allow(dead_code)]
    pub fn with_hooks_setup(self) -> Self {
        let hooks_dir = self.dir.path().join(".githooks");
        std::fs::create_dir_all(&hooks_dir).expect("failed to create .githooks dir");
        git(self.dir.path(), &["config", "core.hooksPath", ".githooks"]);
        std::fs::write(self.dir.path().join(".git-std.toml"), "")
            .expect("failed to write .git-std.toml");
        std::fs::write(self.dir.path().join(".git-blame-ignore-revs"), "")
            .expect("failed to write .git-blame-ignore-revs");
        git(
            self.dir.path(),
            &["config", "blame.ignoreRevsFile", ".git-blame-ignore-revs"],
        );
        self
    }

    /// Write a `.githooks/<name>.hooks` file with the given content.
    // Used by: spec/tests/hooks.rs (not referenced in every test binary)
    #[allow(dead_code)]
    pub fn with_hooks_file(self, name: &str, content: &str) -> Self {
        let hooks_dir = self.dir.path().join(".githooks");
        std::fs::create_dir_all(&hooks_dir).expect("failed to create .githooks dir");
        std::fs::write(hooks_dir.join(format!("{name}.hooks")), content)
            .expect("failed to write hooks file");
        self
    }

    /// Write a `.git-std.toml` config file.
    // Used by: spec/tests/check.rs (lint), spec/tests/bump.rs (not referenced in every test binary)
    #[allow(dead_code)]
    pub fn with_config(self, content: &str) -> Self {
        std::fs::write(self.dir.path().join(".git-std.toml"), content)
            .expect("failed to write config");
        self
    }

    /// Write a minimal `Cargo.toml` with the given version.
    // Used by: spec/tests/bump.rs, spec/tests/changelog.rs (not referenced in every test binary)
    #[allow(dead_code)]
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
    // Used by: spec/tests/bump.rs (not referenced in every test binary)
    #[allow(dead_code)]
    pub fn with_package_json(self, version: &str) -> Self {
        std::fs::write(
            self.dir.path().join("package.json"),
            format!("{{\n  \"name\": \"test-pkg\",\n  \"version\": \"{version}\"\n}}\n"),
        )
        .expect("failed to write package.json");
        self
    }

    /// Write a minimal `project.toml` with the given version.
    // Used by: spec/tests/bump.rs (not referenced in every test binary)
    #[allow(dead_code)]
    pub fn with_project_toml(self, version: &str) -> Self {
        std::fs::write(
            self.dir.path().join("project.toml"),
            format!("name = \"io.driftsys.test\"\nversion = \"{version}\"\nlicense = \"MIT\"\n"),
        )
        .expect("failed to write project.toml");
        self
    }

    /// Write a minimal `project.json` with the given version.
    // Used by: spec/tests/bump.rs (not referenced in every test binary)
    #[allow(dead_code)]
    pub fn with_project_json(self, version: &str) -> Self {
        std::fs::write(
            self.dir.path().join("project.json"),
            format!("{{\n  \"name\": \"io.driftsys.test\",\n  \"version\": \"{version}\"\n}}\n"),
        )
        .expect("failed to write project.json");
        self
    }

    /// Write a minimal `project.yaml` with the given version.
    // Used by: spec/tests/bump.rs (not referenced in every test binary)
    #[allow(dead_code)]
    pub fn with_project_yaml(self, version: &str) -> Self {
        std::fs::write(
            self.dir.path().join("project.yaml"),
            format!("name: io.driftsys.test\nversion: \"{version}\"\nlicense: MIT\n"),
        )
        .expect("failed to write project.yaml");
        self
    }

    /// Create a file, stage it, and commit with the given message.
    // Used by: spec/tests/hooks.rs (not referenced in every test binary)
    #[allow(dead_code)]
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
    // Used by: spec/tests/bump.rs, spec/tests/changelog.rs (not referenced in every test binary)
    #[allow(dead_code)]
    pub fn create_tag(&self, name: &str) -> &Self {
        git(self.dir.path(), &["tag", "-a", name, "-m", name]);
        self
    }

    /// Return the path to the temporary directory.
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Run git in the fixture and return trimmed stdout.
    #[allow(dead_code)]
    pub fn git(&self, args: &[&str]) -> String {
        git(self.path(), args)
    }

    /// Write a fixture file, creating its parent directory when necessary.
    #[allow(dead_code)]
    pub fn write(&self, path: &str, content: &str) {
        let path = self.path().join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("failed to create fixture directory");
        }
        std::fs::write(path, content).expect("failed to write fixture file");
    }

    /// Read a fixture file as UTF-8.
    #[allow(dead_code)]
    pub fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.path().join(path)).expect("failed to read fixture file")
    }

    /// Capture Git facts and all non-Git fixture bytes in deterministic order.
    #[allow(dead_code)]
    pub fn snapshot_state(&self) -> RepoState {
        let mut files = BTreeMap::new();
        collect_files(self.path(), self.path(), &mut files);
        RepoState {
            head: git(self.path(), &["rev-parse", "HEAD"]),
            tags: git(self.path(), &["tag", "--list"]),
            status: git(self.path(), &["status", "--porcelain=v1"]),
            files,
        }
    }

    /// Return the path to the `git-std` binary built by cargo.
    pub fn bin_path() -> PathBuf {
        assert_cmd::cargo::cargo_bin("git-std")
    }
}

fn collect_files(root: &Path, dir: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("failed to read fixture directory")
        .map(|entry| entry.expect("failed to read fixture entry"))
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(root).expect("fixture path below root");
        if relative.starts_with(".git") {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, files);
        } else {
            files.insert(
                relative.to_path_buf(),
                std::fs::read(path).expect("failed to read fixture bytes"),
            );
        }
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
