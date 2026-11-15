"""Blackbox coverage for Cargo-driven release man-page collection."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/release/collect-man-pages.py'


class ManPagesTest(unittest.TestCase):
    def setUp(self):
        self.sandbox = tempfile.TemporaryDirectory()
        self.addCleanup(self.sandbox.cleanup)
        self.root = Path(self.sandbox.name).resolve()
        self.manifest = self.root / 'crates/git-std/Cargo.toml'
        self.target = self.root / 'custom target'
        self.destination = self.root / 'staging'
        self.metadata = {
            'workspace_root': str(self.root),
            'target_directory': str(self.target),
            'packages': [{'name': 'git-std', 'manifest_path': str(self.manifest)}],
        }

    def messages(self, out_dir, manifest=None, package='exact-package-id'):
        return [
            {'reason': 'build-script-executed', 'package_id': package,
             'out_dir': str(out_dir)},
            {'reason': 'compiler-artifact', 'package_id': package,
             'manifest_path': str(manifest or self.manifest),
             'target': {'name': 'git-std', 'kind': ['bin']}},
            {'reason': 'build-finished', 'success': True},
        ]

    def pages(self, out_dir):
        directory = out_dir / 'man'
        directory.mkdir(parents=True)
        (directory / 'git-std.1').write_text('main page')
        (directory / 'git-std-tag.1').write_text('tag page')
        return directory

    def collect(self, messages, cross=False):
        metadata = self.root / 'metadata.json'
        metadata.write_text(json.dumps(self.metadata))
        stream = self.root / 'messages.jsonl'
        stream.write_text('\n'.join(json.dumps(row) for row in messages))
        return subprocess.run(
            [sys.executable, str(SCRIPT), str(metadata), str(stream),
             str(self.destination)] + (['--cross'] if cross else []),
            capture_output=True, text=True,
        )

    def test_old_new_and_custom_layouts_use_reported_output(self):
        for layout in ['release/build/git-std-abc/out',
                       'release/build/git-std/abc/out',
                       'arbitrary build directory/output']:
            with self.subTest(layout=layout):
                output = self.target / layout
                self.pages(output)
                self.destination.mkdir(exist_ok=True)
                (self.destination / 'git-std-stale.1').write_text('stale')
                result = self.collect(self.messages(output))
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(sorted(p.name for p in self.destination.iterdir()),
                                 ['git-std-tag.1', 'git-std.1'])
                self.assertEqual((self.destination / 'git-std.1').read_text(), 'main page')

    def test_only_current_output_pages_and_bytes_are_staged(self):
        current = self.target / 'current/output'
        current_pages = self.pages(current)
        expected = {'git-std.1': b'current main', 'git-std-tag.1': b'current tag'}
        for name, content in expected.items():
            (current_pages / name).write_bytes(content)
        for name in ['stale', 'unrelated']:
            pages = self.pages(self.target / name / 'output')
            (pages / 'git-std.1').write_text(name + ' main')
            (pages / 'git-std-tag.1').write_text(name + ' tag')
            (pages / ('git-std-' + name + '.1')).write_text(name)
        self.destination.mkdir()
        (self.destination / 'notes.txt').write_bytes(b'preserve unrelated file')
        result = self.collect(self.messages(current))
        self.assertEqual(result.returncode, 0, result.stderr)
        expected['notes.txt'] = b'preserve unrelated file'
        self.assertEqual({p.name: p.read_bytes() for p in self.destination.iterdir()},
                         expected)

    def test_cross_maps_container_target_and_workspace_paths(self):
        self.pages(self.target / 'unusual/output')
        result = self.collect(self.messages('/target/unusual/output',
                              '/project/crates/git-std/Cargo.toml'), cross=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.destination / 'git-std-tag.1').is_file())

    def test_cross_accepts_host_mounted_manifest(self):
        self.pages(self.target / 'output')
        result = self.collect(self.messages('/target/output'), cross=True)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_unrelated_same_named_package_cannot_supply_pages(self):
        stale = self.target / 'stale'
        self.pages(stale)
        result = self.collect(self.messages(stale, self.root / 'other/Cargo.toml'))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('git-std binary artifact', result.stderr)
        self.assertFalse(self.destination.exists())

    def test_missing_main_page_does_not_fall_back_to_stale_output(self):
        self.pages(self.target / 'stale')
        current = self.target / 'current'
        self.pages(current).joinpath('git-std.1').unlink()
        result = self.collect(self.messages(current))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('git-std.1', result.stderr)
        self.assertFalse(self.destination.exists())

    def test_failed_or_incomplete_build_is_rejected(self):
        output = self.target / 'output'
        self.pages(output)
        for ending in [[], [{'reason': 'build-finished', 'success': False}]]:
            result = self.collect(self.messages(output)[:-1] + ending)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('successful build', result.stderr)

    def test_ambiguous_build_script_output_is_rejected(self):
        output = self.target / 'output'
        self.pages(output)
        messages = self.messages(output)
        messages.insert(0, dict(messages[0], out_dir=str(self.target / 'other')))
        result = self.collect(messages)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('build-script output', result.stderr)

    def test_unrelated_build_script_is_ignored(self):
        output = self.target / 'output'
        self.pages(output)
        messages = self.messages(output)
        messages.insert(0, dict(messages[0], package_id='unrelated', out_dir='/missing'))
        result = self.collect(messages)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == '__main__':
    unittest.main()
