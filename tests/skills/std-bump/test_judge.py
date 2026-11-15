"""Mutation controls for release-skill evaluation assertions."""
import os
from pathlib import Path
import tempfile
import unittest

from fixtures import build, git, judge


class JudgeControls(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="git-std-judge-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name) / "repository"
        self.binary = Path(os.environ["RELEASE_EVAL_BINARY"]).resolve()

    def test_publication_before_validation_fails_even_if_validation_runs_later(self):
        scenario = {"id": "direct", "expected": "published"}
        initial = build(self.root, scenario, self.binary, None)
        (self.root / "VERSION").write_text("1.0.1\n")
        git(self.root, "add", "VERSION")
        git(self.root, "commit", "-m", "chore(release): 1.0.1")
        git(self.root, "tag", "-a", "v1.0.1", "-m", "1.0.1")
        git(self.root, "push", "origin", "main", "refs/tags/v1.0.1")
        (self.root / ".runtime/verified").write_text(git(self.root, "rev-parse", "HEAD") + "\n")
        self.assertIn("published before successful validation of the exact target", judge(self.root, scenario, initial))

    def test_replacing_dirty_content_without_changing_status_fails(self):
        scenario = {"id": "dirty", "expected": "unchanged"}
        initial = build(self.root, scenario, self.binary, None)
        (self.root / "feature.txt").write_text("different uncommitted content\n")
        self.assertEqual(initial["status"], git(self.root, "status", "--porcelain"))
        self.assertIn("caller changes were altered", judge(self.root, scenario, initial))

    def test_empty_preparation_branch_is_not_a_release(self):
        scenario = {"id": "two-step", "expected": "prepared"}
        initial = build(self.root, scenario, self.binary, None)
        git(self.root, "push", "origin", "HEAD:refs/heads/release")
        failures = judge(self.root, scenario, initial)
        self.assertIn("preparation branch does not contain the release version", failures)
        self.assertIn("preparation revision was not validated", failures)


if __name__ == "__main__":
    unittest.main()
