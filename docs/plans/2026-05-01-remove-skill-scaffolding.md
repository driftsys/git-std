# Remove Skill Scaffolding from `git std init` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove all skill scaffolding from `git std init`, restructure `skills/`
to the agentskills.io format so `npx skills add driftsys/git-std` works, and add
a one-liner installation note to the docs.

**Architecture:** Three independent deletions (skills/ layout, scaffold.rs,
mod.rs) plus two doc additions. No new abstractions — this is a net subtraction
of code. Tests update path strings only; no new test logic needed.

**Tech Stack:** Rust (cargo/clippy), `just check`, `git std commit`

---

## File map

| File                                      | Change                                                              |
| ----------------------------------------- | ------------------------------------------------------------------- |
| `skills/std-commit.md`                    | Delete (content moves to `skills/std-commit/SKILL.md`)              |
| `skills/std-bump.md`                      | Delete (content moves to `skills/std-bump/SKILL.md`)                |
| `skills/std-commit/SKILL.md`              | Create                                                              |
| `skills/std-bump/SKILL.md`                | Create                                                              |
| `crates/git-std/src/cli/init/scaffold.rs` | Remove 3 functions + import + update doc + fix `include_str!` paths |
| `crates/git-std/src/cli/init/mod.rs`      | Remove 4 constants + 3 imports + step 8 + skill loop in refresh     |
| `docs/README.md`                          | Add AI agent skills bullet to Features                              |
| `docs/getting-started.md`                 | Add AI agent skills section                                         |

---

## Task 1: Restructure `skills/` to agentskills.io format

**Files:**

- Delete: `skills/std-commit.md`
- Delete: `skills/std-bump.md`
- Create: `skills/std-commit/SKILL.md`
- Create: `skills/std-bump/SKILL.md`
- Modify: `crates/git-std/src/cli/init/scaffold.rs` (2 `include_str!` paths)

- [ ] **Step 1: Verify current content**

  ```bash
  cat skills/std-commit.md | head -5
  cat skills/std-bump.md | head -5
  ```

  Expected: both start with `---\nname: std-commit` / `---\nname: std-bump`
  frontmatter.

- [ ] **Step 2: Move files into subdirectories**

  ```bash
  mkdir -p skills/std-commit skills/std-bump
  mv skills/std-commit.md skills/std-commit/SKILL.md
  mv skills/std-bump.md   skills/std-bump/SKILL.md
  ```

- [ ] **Step 3: Update `include_str!` paths in scaffold.rs unit tests**

  In `crates/git-std/src/cli/init/scaffold.rs`, find and replace both
  `include_str!` paths in the `#[cfg(test)]` block:

  ```rust
  // Before
  let s = include_str!("../../../../../skills/std-commit.md");
  // After
  let s = include_str!("../../../../../skills/std-commit/SKILL.md");
  ```

  ```rust
  // Before
  let s = include_str!("../../../../../skills/std-bump.md");
  // After
  let s = include_str!("../../../../../skills/std-bump/SKILL.md");
  ```

  There are three test functions touching these paths —
  `std_commit_skill_has_frontmatter`, `std_commit_skill_includes_message_guidelines`,
  and `std_bump_skill_has_frontmatter`. Update all occurrences.

- [ ] **Step 4: Run `just check` and confirm it passes**

  ```bash
  just check
  ```

  Expected: all tests green, no clippy warnings, no lint errors.

- [ ] **Step 5: Commit**

  ```bash
  git add skills/ crates/git-std/src/cli/init/scaffold.rs
  git std commit --type refactor --scope git-std \
    --message "restructure skills/ to agentskills.io subdirectory format"
  ```

---

## Task 2: Remove skill scaffolding from scaffold.rs and mod.rs

Do these two files in one commit — removing the functions from `scaffold.rs`
before removing the calls from `mod.rs` (or vice versa) would leave the code
in a non-compiling state mid-task.

**Files:**

- Modify: `crates/git-std/src/cli/init/scaffold.rs`
- Modify: `crates/git-std/src/cli/init/mod.rs`

### scaffold.rs

- [ ] **Step 1: Remove the module-doc line that mentions skill files**

  In `scaffold.rs`, the top-level module doc currently reads:

  ```rust
  //! Owns: `.git-std.toml` config, lifecycle hook templates, agent skill files
  //! and their `.claude/skills/` symlinks.
  ```

  Replace with:

  ```rust
  //! Owns: `.git-std.toml` config and lifecycle hook templates.
  ```

- [ ] **Step 2: Delete `write_skill_source()`**

  Remove the entire function (lines ~88–140). It starts with:

  ```rust
  /// Create a symlink from `.agents/skills/<name>/SKILL.md` to `../../skills/<name>.md`.
  pub fn write_skill_source(
  ```

  and ends with `FileResult::Created\n}`.

- [ ] **Step 3: Delete `write_skill_symlink()`**

  Remove the entire function (lines ~142–178). It starts with:

  ```rust
  /// Create a `.claude/skills/` symlink pointing back to `.agents/skills/`.
  pub fn write_skill_symlink(root: &Path, link: &str, target: &str, force: bool) -> FileResult {
  ```

  and ends with `FileResult::Created\n}`.

- [ ] **Step 4: Delete `skill_definitions()`**

  Remove the entire function (lines ~179–193). It starts with:

  ```rust
  /// Return all skill definitions for scaffolding.
  ///
  /// Each tuple: `(skill_name, skill_dir, claude_link)`.
  pub fn skill_definitions() -> Vec<(&'static str, &'static str, &'static str)> {
  ```

  and ends with `]\n}`.

### mod.rs

- [ ] **Step 5: Remove the four skill constants**

  Delete these four lines from the constants block:

  ```rust
  const AGENTS_SKILL_COMMIT_DIR: &str = ".agents/skills/std-commit";
  const AGENTS_SKILL_BUMP_DIR: &str = ".agents/skills/std-bump";
  const CLAUDE_SKILL_COMMIT: &str = ".claude/skills/std-commit";
  const CLAUDE_SKILL_BUMP: &str = ".claude/skills/std-bump";
  ```

- [ ] **Step 6: Remove skill imports from the `scaffold` use statement**

  The current import block reads:

  ```rust
  use scaffold::{
      generate_lifecycle_hook_template, skill_definitions, write_config_file, write_skill_source,
      write_skill_symlink,
  };
  ```

  Replace with:

  ```rust
  use scaffold::{generate_lifecycle_hook_template, write_config_file};
  ```

- [ ] **Step 7: Remove step 8 from `init()` and update its doc comment**

  In `mod.rs`, the module-level doc lists steps 1–10. Remove step 8:

  ```rust
  //! 8. Scaffold agent skills in `.agents/skills/` with `.claude/skills/` symlinks.
  ```

  and renumber steps 9→8 and 10→9:

  ```rust
  //! 8. Append post-clone section to README/AGENTS (if found).
  //! 9. Stage everything.
  ```

  Also remove the `run()` doc comment line:

  ```rust
  /// When `refresh` is true, only updates skill files and merges config
  /// defaults — skips hook setup, bootstrap, and README markers.
  ```

  Replace with:

  ```rust
  /// When `refresh` is true, only merges config defaults — skips hook setup,
  /// bootstrap, and README markers.
  ```

  Then delete the entire step 8 block from `init()`:

  ```rust
  // ── Step 8: scaffold agent skills ───────────────────────────────────────
  for (skill_name, skill_dir, claude_link) in skill_definitions() {
      // Create symlink in .agents/skills/<name>/SKILL.md → ../../skills/<name>.md
      match write_skill_source(&root, skill_dir, skill_name, force) {
          FileResult::Created => {
              staged.push(skill_dir);
              ui::info(&format!(
                  "{}  {skill_dir}/SKILL.md → ../../skills/{skill_name}.md created",
                  ui::pass()
              ));
          }
          FileResult::Skipped => {}
          FileResult::Error => return 1,
      }
      // Create symlink in .claude/skills/<name> → ../../.agents/skills/<name>
      match write_skill_symlink(&root, claude_link, skill_dir, force) {
          FileResult::Created => {
              staged.push(claude_link);
              ui::info(&format!(
                  "{}  {claude_link} → {skill_dir} created",
                  ui::pass()
              ));
          }
          FileResult::Skipped => {}
          FileResult::Error => return 1,
      }
  }
  ```

- [ ] **Step 8: Remove the skill loop from `run_refresh()`**

  Delete the `// ── Update skill files` block from `run_refresh()`:

  ```rust
  // ── Update skill files (force-overwrite to get latest) ──────────────────
  for (skill_name, skill_dir, claude_link) in skill_definitions() {
      match write_skill_source(root, skill_dir, skill_name, true) {
          FileResult::Created => {
              staged.push(skill_dir);
              ui::info(&format!("{}  {skill_dir}/SKILL.md refreshed", ui::pass()));
          }
          FileResult::Skipped => {}
          FileResult::Error => return 1,
      }
      match write_skill_symlink(root, claude_link, skill_dir, force) {
          FileResult::Created => {
              staged.push(claude_link);
              ui::info(&format!(
                  "{}  {claude_link} → {skill_dir} created",
                  ui::pass()
              ));
          }
          FileResult::Skipped => {}
          FileResult::Error => return 1,
      }
  }
  ```

- [ ] **Step 9: Run `just check` and confirm it passes**

  ```bash
  just check
  ```

  Expected: all tests green, no clippy warnings (unused imports/constants are the
  most likely failure — verify they're all gone), no lint errors.

- [ ] **Step 10: Commit**

  ```bash
  git add crates/git-std/src/cli/init/scaffold.rs \
          crates/git-std/src/cli/init/mod.rs
  git std commit --type refactor --scope git-std \
    --message "remove skill scaffolding from init"
  ```

---

## Task 3: Add AI agent skills docs

**Files:**

- Modify: `docs/README.md`
- Modify: `docs/getting-started.md`

- [ ] **Step 1: Add bullet to `docs/README.md` Features section**

  In the Features list (after the "Shell completions" bullet), add:

  ```markdown
  - **AI agent skills** — install commit and bump skills for your AI coding
    agent with `npx skills add driftsys/git-std`
  ```

- [ ] **Step 2: Add section to `docs/getting-started.md`**

  After the "## Set up hooks" section (after line 38), insert:

  ```markdown
  ## AI agent skills

  Install the `std-commit` and `std-bump` skills for your AI coding agent:

  ​`bash
  npx skills add driftsys/git-std
  ​`
  ```

- [ ] **Step 3: Run `just check`**

  ```bash
  just check
  ```

  Expected: markdownlint passes, all tests green.

- [ ] **Step 4: Commit**

  ```bash
  git add docs/README.md docs/getting-started.md
  git std commit --type docs --scope git-std \
    --message "document npx skills add one-liner for agent skills"
  ```

---

## Task 4: Open PR and close #488

- [ ] **Step 1: Push branch**

  ```bash
  git push origin HEAD:refactor/488-remove-skill-scaffolding -u
  ```

- [ ] **Step 2: Open PR**

  ```bash
  gh pr create \
    --title "refactor(git-std): remove skill scaffolding, adopt agentskills.io layout" \
    --base main \
    --body "## Summary

  - Restructure \`skills/\` to agentskills.io format (\`skills/<name>/SKILL.md\`) so \`npx skills add driftsys/git-std\` works
  - Remove all skill scaffolding from \`git std init\` and \`init --refresh\` — skill lifecycle delegated to \`npx skills\` / future \`upskill\`
  - Add \`npx skills add driftsys/git-std\` one-liner to README and getting-started
  - Skill content tests (\`std_commit_skill_has_frontmatter\` etc.) kept and path-updated — they guard source file quality for consumers

  Closes #488"
  ```

- [ ] **Step 3: Monitor CI and merge when green**

  ```bash
  gh pr checks <pr-number> --watch
  gh pr merge <pr-number> --squash --delete-branch
  ```

- [ ] **Step 4: Confirm #488 closed**

  ```bash
  gh issue view 488 --json state --jq '.state'
  ```

  Expected: `"CLOSED"` (auto-closed by `Closes #488` in the PR body).

- [ ] **Step 5: Sync local main**

  ```bash
  git fetch origin main && git reset --hard origin/main
  ```
