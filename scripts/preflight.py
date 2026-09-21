#!/usr/bin/env python3
"""Run the cheapest relevant Sindri checks before a push.

The preflight discovers changed files from git, maps Rust files to workspace
packages through Cargo metadata, and runs narrow checks before CI has to do it.
It is intentionally not a replacement for render, browser, dependency, or WASM
checks required by AGENTS.md for changes that affect those surfaces.
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path


def run(root: Path, command: list[str], *, cwd: Path | None = None) -> bool:
    shown = " ".join(command)
    relative = (cwd or root).relative_to(root) if (cwd or root) != root else Path(".")
    prefix = "" if relative == Path(".") else f"(cd {relative} && "
    suffix = "" if not prefix else ")"
    print(f"\n==> {prefix}{shown}{suffix}", flush=True)
    return subprocess.run(command, cwd=cwd or root, check=False).returncode == 0


def capture(root: Path, command: list[str]) -> str:
    result = subprocess.run(
        command, cwd=root, check=True, capture_output=True, text=True
    )
    return result.stdout


def changed_files(root: Path, base: str) -> list[Path]:
    merge_base = capture(root, ["git", "merge-base", base, "HEAD"]).strip()
    output = capture(
        root,
        ["git", "diff", "--name-only", "--diff-filter=ACMRT", merge_base, "HEAD"],
    )
    return [Path(line) for line in output.splitlines() if line]


def workspace_packages(root: Path) -> list[tuple[Path, str]]:
    metadata = json.loads(
        capture(root, ["cargo", "metadata", "--no-deps", "--format-version", "1"])
    )
    members = set(metadata["workspace_members"])
    packages = []
    for package in metadata["packages"]:
        if package["id"] not in members:
            continue
        manifest = Path(package["manifest_path"]).resolve()
        packages.append((manifest.parent, package["name"]))
    return sorted(packages, key=lambda item: len(item[0].parts), reverse=True)


def changed_packages(
    root: Path, files: list[Path], packages: list[tuple[Path, str]]
) -> list[str]:
    names = set()
    for relative in files:
        absolute = (root / relative).resolve()
        for package_root, name in packages:
            if absolute == package_root or package_root in absolute.parents:
                names.add(name)
                break
    return sorted(names)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--base",
        default="origin/main",
        help="branch/ref to diff against (default: origin/main)",
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="show discovered files/packages without running checks",
    )
    args = parser.parse_args()

    root = Path(
        capture(Path.cwd(), ["git", "rev-parse", "--show-toplevel"]).strip()
    )
    try:
        files = changed_files(root, args.base)
        packages = workspace_packages(root)
    except subprocess.CalledProcessError as error:
        print(f"preflight discovery failed: {error}", file=sys.stderr)
        return 2

    rust_packages = changed_packages(root, files, packages)
    decay_files = sorted(str(path) for path in files if path.suffix == ".decay")
    workspace_wide = any(
        path in {Path("Cargo.toml"), Path("rust-toolchain.toml")} for path in files
    )

    print(f"Changed files: {len(files)}")
    print(
        "Rust scope: "
        + ("workspace" if workspace_wide else ", ".join(rust_packages) or "none")
    )
    print("Decay scripts: " + (", ".join(decay_files) or "none"))

    if args.list or not files:
        return 0

    ok = run(root, ["cargo", "fmt", "--all", "--check"])
    ok = run(root, ["scripts/check-file-size.py"]) and ok

    if decay_files:
        ok = run(
            root,
            [
                "cargo",
                "run",
                "--quiet",
                "--package",
                "decay-lsp",
                "--",
                "--check",
                *decay_files,
            ],
        ) and ok

    scopes = [None] if workspace_wide else rust_packages
    for package in scopes:
        check = ["cargo", "check"]
        test = ["cargo", "test"]
        if package is None:
            check += ["--workspace"]
            test += ["--workspace"]
        else:
            check += ["-p", package]
            test += ["-p", package]
        check += ["--all-targets", "--all-features"]
        test += ["--all-features"]
        ok = run(root, check) and ok
        ok = run(root, test) and ok

    if not ok:
        print("\nPreflight failed. Fix every failure above before pushing.", file=sys.stderr)
        return 1

    print("\nPreflight passed.")
    if rust_packages or workspace_wide:
        print("Run applicable WASM/render/browser/dependency checks from AGENTS.md too.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
