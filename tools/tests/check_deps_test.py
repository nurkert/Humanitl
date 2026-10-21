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


def core_with_source(text: str) -> subprocess.CompletedProcess[str]:
    """humanitl-audit with one source file holding `text` (HUM-155)."""
    with tempfile.TemporaryDirectory() as crate:
        src = Path(crate) / "src"
        src.mkdir()
        (src / "lib.rs").write_text(text, encoding="utf-8")
        package = pkg("humanitl-audit", [("humanitl-core", None)])
        package["manifest_path"] = str(Path(crate) / "Cargo.toml")
        return run([package])


# Jede Zeile ein Weg, wie IO in eine Kern-Crate käme (HUM-155).
IO_SOURCES = [
    ("std::fs path", 'pub fn f() -> bool { std::fs::metadata("/").is_ok() }\n'),
    ("grouped import", "use std::{fs, io};\n"),
    ("grouped import over lines", "use std::{\n    collections::{HashMap},\n    fs,\n};\n"),
    ("renamed in a group", "use std::{fs as f};\n"),
    ("bare fs path", 'pub fn f() { let _ = fs::read("x"); }\n'),
    ("std glob", "use std::*;\n"),
    ("std alias", "use std as s;\n"),
    ("File::open", 'pub fn f() { let _ = File::open("x"); }\n'),
    ("OpenOptions", "pub fn f() { let _ = OpenOptions::new(); }\n"),
    ("exists", 'pub fn f(p: &Path) -> bool { p.exists() }\n'),
    ("metadata", "pub fn f(p: &Path) { let _ = p.metadata(); }\n"),
    ("read_dir", "pub fn f(p: &Path) { let _ = p.read_dir(); }\n"),
    ("canonicalize", "pub fn f(p: &Path) { let _ = p.canonicalize(); }\n"),
    ("tokio", "pub fn f() { tokio::spawn(async {}); }\n"),
    ("thread spawn", "use std::thread;\npub fn f() { thread::spawn(|| ()); }\n"),
    ("thread builder", "pub fn f() { let _ = std::thread::Builder::new(); }\n"),
    ("grouped thread", "use std::{thread};\n"),
    ("process", 'pub fn f() { let _ = std::process::Command::new("x"); }\n'),
    ("socket", "use std::net::TcpStream;\n"),
    ("socket in a group", "use std::net::{IpAddr, UdpSocket};\n"),
]

# Und was eine Kern-Crate weiter darf.
CLEAN_SOURCES = [
    ("comment", "// Die Datei öffnet std::fs im Adapter.\npub fn f() {}\n"),
    ("block comment", "/* use std::fs; */\npub fn f() {}\n"),
    ("address values", "use std::net::{IpAddr, Ipv4Addr};\nuse std::net::IpAddr as A;\n"),
    ("readers", "use std::io::{self, BufRead};\nuse std::path::Path;\n"),
]


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

    # Der Kern bekommt keine Laufzeit und keinen Systemaufruf zurück (HUM-155).
    for forbidden in ("tokio", "rustix", "libc"):
        core_dep = run([pkg("humanitl-audit", [("humanitl-core", None), (forbidden, None)])])
        if core_dep.returncode != 1 or f"humanitl-audit depends on {forbidden}" not in core_dep.stderr:
            print(f"FAIL core_{forbidden}_fails: rc={core_dep.returncode} err={core_dep.stderr}")
            failures += 1

    core_dev = run([pkg("humanitl-audit", [("humanitl-core", None), ("tokio", "dev")])])
    if core_dev.returncode != 0:
        print(f"FAIL core_dev_tokio_ignored: {core_dev.stderr}")
        failures += 1

    adapter = run([pkg("humanitl-audit-log", [("humanitl-audit", None), ("tokio", None)])])
    if adapter.returncode != 0:
        print(f"FAIL adapter_may_use_tokio: {adapter.stderr}")
        failures += 1

    for label, text in IO_SOURCES:
        opens = core_with_source(text)
        if opens.returncode != 1 or "does IO" not in opens.stderr:
            print(f"FAIL core_source_fails[{label}]: rc={opens.returncode} err={opens.stderr}")
            failures += 1

    for label, text in CLEAN_SOURCES:
        clean_core = core_with_source(text)
        if clean_core.returncode != 0:
            print(f"FAIL core_source_passes[{label}]: {clean_core.stderr}")
            failures += 1

    for forbidden in ("nix", "fs-err", "memmap2", "mio", "async-std", "tempfile"):
        core_dep = run([pkg("humanitl-rules", [(forbidden, None)])])
        if core_dep.returncode != 1 or f"depends on {forbidden}" not in core_dep.stderr:
            print(f"FAIL core_{forbidden}_fails: rc={core_dep.returncode} err={core_dep.stderr}")
            failures += 1

    print("all check_deps tests passed" if not failures else f"{failures} test(s) failed")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
