#!/usr/bin/env python3
"""Tests for tools/check_deps.py (run: python3 tools/tests/check_deps_test.py)."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "tools" / "check_deps.py"
ALLOW = ROOT / "tools" / "deps-allow.toml"


def run(packages: list[dict]) -> subprocess.CompletedProcess[str]:
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as handle:
        json.dump({"packages": packages}, handle)
        path = handle.name
    return subprocess.run(
        [sys.executable, str(CHECKER), path, str(ALLOW)],
        capture_output=True,
        text=True,
        check=False,
    )


def pkg(name: str, deps: list[tuple[str, str | None]]) -> dict:
    return {
        "name": name,
        "dependencies": [{"name": d, "kind": k} for d, k in deps],
    }


def main() -> int:
    failures = 0

    clean = run([pkg("humanitl-proxy", [("humanitl-core", None), ("tokio", None)])])
    if clean.returncode != 0:
        print(f"FAIL allowed_dependency_passes: {clean.stderr}")
        failures += 1

    bad = run([pkg("humanitl-core", [("humanitl-rules", None)])])
    if bad.returncode != 1 or "humanitl-core -> humanitl-rules not allowed" not in bad.stderr:
        print(f"FAIL forbidden_dependency_fails: rc={bad.returncode} err={bad.stderr}")
        failures += 1

    dev = run([pkg("humanitl-core", [("humanitl-rules", "dev")])])
    if dev.returncode != 0:
        print(f"FAIL dev_dependency_ignored: {dev.stderr}")
        failures += 1

    unlisted = run([pkg("humanitl-newthing", [])])
    if unlisted.returncode != 1 or "not listed" not in unlisted.stderr:
        print(f"FAIL unlisted_crate_fails: rc={unlisted.returncode} err={unlisted.stderr}")
        failures += 1

    exempt = run([pkg("humanitld", [("humanitl-proxy", None), ("humanitl-ipc", None)])])
    if exempt.returncode != 0:
        print(f"FAIL exempt_crate_passes: {exempt.stderr}")
        failures += 1

    print("all check_deps tests passed" if not failures else f"{failures} test(s) failed")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
