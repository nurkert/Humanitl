#!/usr/bin/env python3
"""Check internal crate dependencies against tools/deps-allow.toml.

Reads `cargo metadata --no-deps` output and fails when a workspace crate
depends on an internal crate that the allow table does not list. Dev
dependencies are ignored: tests may depend on anything.
"""

from __future__ import annotations

import json
import sys
import tomllib


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

    for name in sorted(unknown):
        print(f"crate not listed in deps-allow.toml: {name}", file=sys.stderr)
    for line in sorted(violations):
        print(line, file=sys.stderr)
    return 1 if violations or unknown else 0


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("usage: check_deps.py <cargo-metadata.json> <deps-allow.toml>", file=sys.stderr)
        raise SystemExit(2)
    raise SystemExit(main(sys.argv[1], sys.argv[2]))
