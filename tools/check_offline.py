#!/usr/bin/env python3
"""Keep the crates that must never reach the network free of network access.

`humanitl-catalog` answers from files that ship with the program and never
asks anyone (HUM-031). That is a property of its dependency graph and of its
own sources. This check reads the full `cargo metadata` output with its
resolved graph (filtered to the platform being built, see
tools/check-deps.sh) and, for every crate listed in tools/offline-crates.toml:

- walks its normal dependencies, transitively, and fails on any crate from the
  network list, naming the shortest path to it;
- reads every non-comment line of its `src/` and fails on the network API of
  the standard library or a raw socket through `libc`, which need no crate.

Dev and build dependencies are ignored: a test may open a socket, and a build
script runs on the machine that builds, not in the shipped crate.

Usage: check_offline.py <metadata.json> <offline-crates.toml>
"""

from __future__ import annotations

import json
import re
import sys
import tomllib
from collections import deque
from pathlib import Path


def is_normal(dep: dict) -> bool:
    """Whether a resolved edge is a normal dependency."""
    return any(kind.get("kind") is None for kind in dep.get("dep_kinds", []))


def normal_closure(meta: dict, root: str) -> dict[str, list[str]]:
    """Every package reachable from `root` over normal edges, with a shortest path to it."""
    nodes = {node["id"]: node for node in meta["resolve"]["nodes"]}
    names = {package["id"]: package["name"] for package in meta["packages"]}
    paths = {root: [names.get(root, root)]}
    queue = deque([root])
    while queue:
        current = queue.popleft()
        for dep in nodes.get(current, {}).get("deps", []):
            target = dep["pkg"]
            if target in paths or not is_normal(dep):
                continue
            paths[target] = [*paths[current], names.get(target, target)]
            queue.append(target)
    return paths


def source_hits(manifest: str, pattern: re.Pattern[str]) -> list[str]:
    """Every non-comment line under the crate's `src/` that matches `pattern`."""
    hits: list[str] = []
    src = Path(manifest).parent / "src"
    for path in sorted(src.rglob("*.rs")):
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            if line.lstrip().startswith("//"):
                continue
            if pattern.search(line):
                hits.append(f"{path}:{number}: {line.strip()}")
    return hits


def main(meta_path: str, config_path: str) -> int:
    with open(meta_path, "rb") as handle:
        meta = json.load(handle)
    with open(config_path, "rb") as handle:
        config = tomllib.load(handle)
    offline = config.get("offline", {}).get("crates", [])
    forbidden = set(config.get("network", {}).get("crates", []))
    source_rule = config.get("source", {}).get("forbidden")
    pattern = re.compile(source_rule) if source_rule else None
    if not meta.get("resolve"):
        print("check_offline: the metadata has no resolved graph (ran with --no-deps?)", file=sys.stderr)
        return 1

    workspace = {
        package["name"]: package
        for package in meta.get("packages", [])
        if package.get("source") is None
    }
    names = {package["id"]: package["name"] for package in meta.get("packages", [])}
    failures: list[str] = []
    for crate in offline:
        package = workspace.get(crate)
        if package is None:
            failures.append(f"{crate}: listed as offline but not a crate of this workspace")
            continue
        for reached, path in sorted(normal_closure(meta, package["id"]).items(), key=lambda item: item[1]):
            name = names.get(reached, reached)
            if name in forbidden:
                failures.append(f"{crate} reaches the network crate {name}: {' -> '.join(path)}")
        if pattern is not None and package.get("manifest_path"):
            for hit in source_hits(package["manifest_path"], pattern):
                failures.append(f"{crate} uses a network API in its own source: {hit}")

    for line in failures:
        print(line, file=sys.stderr)
    if failures:
        return 1
    print(f"check_offline: {', '.join(offline)} free of {len(forbidden)} network crates and of network APIs in its source")
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        sys.exit(2)
    sys.exit(main(sys.argv[1], sys.argv[2]))
