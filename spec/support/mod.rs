use std::path::{Path, PathBuf};

/// Fluent builder for test git repositories.
///
/// Creates a temporary directory with a git repository, configures
/// user identity, and provides methods to set up hooks files,
/// config, commits, and tags.
pub struct TestRepo {
    dir: tempfile::TempDir,
    repo: git2::Repository,
    file_counter: usize,
}

impl TestRepo {
    /// Create a new test repository with `git init` and user config.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let repo = git2::Repository::init(dir.path()).expect("failed to init repo");

        {
            let mut config = repo.config().expect("failed to get config");
            config
                .set_str("user.name", "Test")
                .expect("failed to set user.name");
            config
                .set_str("user.email", "test@test.com")
                .expect("failed to set user.email");
        }

        Self {
            dir,
            repo,
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

    /// Stage all files in the working directory.
    pub fn stage_all(&self) -> &Self {
        let mut index = self.repo.index().expect("failed to get index");
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .expect("failed to stage all files");
        index.write().expect("failed to write index");
        self
    }

    /// Create a file, stage it, and commit with the given message.
    pub fn add_commit(&mut self, message: &str) -> &mut Self {
        self.file_counter += 1;
        let filename = format!("file-{}.txt", self.file_counter);
        std::fs::write(self.dir.path().join(&filename), message)
            .expect("failed to write commit file");

        let mut index = self.repo.index().expect("failed to get index");
        index
            .add_path(Path::new(&filename))
            .expect("failed to stage file");
        index.write().expect("failed to write index");
        let tree_oid = index.write_tree().expect("failed to write tree");
        let sig = self.repo.signature().expect("failed to get signature");

        // Scope the tree borrow so it doesn't conflict with &mut self.
        {
            let tree = self.repo.find_tree(tree_oid).expect("failed to find tree");
            if let Ok(head) = self.repo.head() {
                let parent = head.peel_to_commit().expect("failed to peel to commit");
                self.repo
                    .commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])
                    .expect("failed to create commit");
            } else {
                self.repo
                    .commit(Some("HEAD"), &sig, &sig, message, &tree, &[])
                    .expect("failed to create initial commit");
            }
        }

        self
    }

    /// Create an annotated tag at HEAD.
    pub fn create_tag(&self, name: &str) -> &Self {
        let sig = self.repo.signature().expect("failed to get signature");
        let head = self
            .repo
            .head()
            .expect("failed to get HEAD")
            .peel_to_commit()
            .expect("failed to peel to commit");
        self.repo
            .tag(name, head.as_object(), &sig, name, false)
            .expect("failed to create tag");
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
