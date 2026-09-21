#!/usr/bin/env python3
import tempfile
import unittest
from pathlib import Path

from scripts import preflight


class PreflightTests(unittest.TestCase):
    def test_maps_files_to_deepest_workspace_package(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            packages = [
                (root / "crates" / "outer" / "tool", "tool"),
                (root / "crates" / "outer", "outer"),
            ]
            files = [
                Path("crates/outer/src/lib.rs"),
                Path("crates/outer/tool/src/main.rs"),
            ]
            self.assertEqual(
                preflight.changed_packages(root, files, packages),
                ["outer", "tool"],
            )

    def test_ignores_files_outside_workspace_packages(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            packages = [(root / "crates" / "engine", "engine")]
            self.assertEqual(
                preflight.changed_packages(root, [Path("docs/diagnostics.md")], packages),
                [],
            )


if __name__ == "__main__":
    unittest.main()
