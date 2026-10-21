#!/usr/bin/env python3
"""Check internal crate dependencies against tools/deps-allow.toml.

Reads `cargo metadata --no-deps` output and fails when a workspace crate
depends on an internal crate that the allow table does not list. Dev
dependencies are ignored: tests may depend on anything.

The crates in `[core]` (docs/ARCHITECTURE.md 2, HUM-155) are checked further:
no normal dependency on a crate from `forbidden` (a runtime, raw system calls,
temporary files), and no match of a `source` pattern in their `src/` once
comments are removed (opening a file, `tokio`, a thread or process of their
own, a socket). Each file is searched as a whole, so an import spread over
several lines (`use std::{\n    fs,\n};`) is found as well.

Deliberate gaps: build dependencies are skipped on purpose, because a build
script runs on the machine that builds and is not part of the shipped core.
Modules pulled in with `#[path = "..."]` from outside `src/` are not scanned.
The patterns are text, not a parse of the Rust source, and stay evadable by
someone who means to; a check over the syntax tree (for example with `syn`)
would be the real fix.
"""

from __future__ import annotations

import json
import re
import sys
import tomllib
from pathlib import Path


def main(meta_path: str, allow_path: str) -> int:
    with open(meta_path, "rb") as handle:
        meta = json.load(handle)
    with open(allow_path, "rb") as handle:
        allow_doc = tomllib.load(handle)

    allow = allow_doc.get("allow", {})
    exempt = set(allow_doc.get("exempt", {}).get("crates", []))
    violations: list[str] = []
    unknown: list[str] = []

    for package in meta.get("packages", []):
        name = package["name"]
        if not name.startswith("humanitl") or name in exempt:
            continue
        if name not in allow:
            unknown.append(name)
            continue
        permitted = set(allow[name])
        for dep in package.get("dependencies", []):
            dep_name = dep["name"]
            if not dep_name.startswith("humanitl"):
                continue
            if dep.get("kind") in ("dev", "build"):
                continue
            if dep_name not in permitted:
                violations.append(f"{name} -> {dep_name} not allowed")

    violations.extend(core_violations(meta, allow_doc.get("core", {})))

    for name in sorted(unknown):
        print(f"crate not listed in deps-allow.toml: {name}", file=sys.stderr)
    for line in sorted(violations):
        print(line, file=sys.stderr)
    return 1 if violations or unknown else 0


def core_violations(meta: dict, core: dict) -> list[str]:
    """Dependencies and source lines that bring IO or async into a core crate."""
    crates = set(core.get("crates", []))
    forbidden = set(core.get("forbidden", []))
    rule = core.get("source")
    if isinstance(rule, list):
        rule = "|".join(f"(?:{part})" for part in rule)
    pattern = re.compile(rule) if rule else None
    found: list[str] = []
    for package in meta.get("packages", []):
        name = package["name"]
        if name not in crates:
            continue
        for dep in package.get("dependencies", []):
            if dep.get("kind") in ("dev", "build"):
                continue
            if dep["name"] in forbidden:
                found.append(f"core crate {name} depends on {dep['name']} (no IO, no async in the core)")
        if pattern is None or "manifest_path" not in package:
            continue
        src = Path(package["manifest_path"]).parent / "src"
        for path in sorted(src.rglob("*.rs")):
            code = strip_comments(path.read_text(encoding="utf-8"))
            for match in pattern.finditer(code):
                number = code.count("\n", 0, match.start()) + 1
                line = code.splitlines()[number - 1].strip()
                found.append(f"core crate {name} does IO: {path}:{number}: {line}")
    return found


def strip_comments(text: str) -> str:
    """`text` with line and block comments blanked; line numbers stay where they were."""
    text = re.sub(r"/\*.*?\*/", lambda block: re.sub(r"[^\n]", " ", block.group()), text, flags=re.S)
    return re.sub(r"//[^\n]*", "", text)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("usage: check_deps.py <cargo-metadata.json> <deps-allow.toml>", file=sys.stderr)
        raise SystemExit(2)
    raise SystemExit(main(sys.argv[1], sys.argv[2]))
