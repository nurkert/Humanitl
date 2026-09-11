#!/usr/bin/env python3
"""Tests for tools/check_offline.py (run: python3 tools/tests/check_offline_test.py)."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "tools" / "check_offline.py"
CONFIG = ROOT / "tools" / "offline-crates.toml"

WORKSPACE = {"humanitl-catalog", "humanitl-core"}


def kinds_of(spec: object) -> list[dict]:
    """`dep_kinds` as cargo writes them: a kind, or a list of kind/target pairs."""
    if isinstance(spec, list):
        return spec
    return [{"kind": spec, "target": None}]


def graph(edges: dict[str, list[tuple[str, object]]], manifests: dict[str, Path] | None = None) -> dict:
    """Metadata with a resolved graph: crate -> [(dependency, kind or dep_kinds)].

    Like cargo, the edge carries the extern name with `_` for `-`; the package
    name stays hyphenated.
    """
    manifests = manifests or {}
    names = set(edges) | {dep for deps in edges.values() for dep, _ in deps}
    return {
        "packages": [
            {
                "id": f"{name} 0.0.0",
                "name": name,
                "source": None if name in WORKSPACE else "registry",
                "manifest_path": str(manifests[name]) if name in manifests else None,
            }
            for name in sorted(names)
        ],
        "resolve": {
            "nodes": [
                {
                    "id": f"{name} 0.0.0",
                    "deps": [
                        {"name": dep.replace("-", "_"), "pkg": f"{dep} 0.0.0", "dep_kinds": kinds_of(kind)}
                        for dep, kind in edges.get(name, [])
                    ],
                }
                for name in sorted(names)
            ]
        },
    }


def run(meta: dict) -> subprocess.CompletedProcess[str]:
    with tempfile.TemporaryDirectory() as scratch:
        path = Path(scratch) / "meta.json"
        path.write_text(json.dumps(meta), encoding="utf-8")
        return subprocess.run(
            [sys.executable, str(CHECKER), str(path), str(CONFIG)],
            capture_output=True,
            text=True,
            check=False,
        )


def crate_with_source(scratch: Path, text: str, file: str = "lib.rs") -> Path:
    """A crate directory whose `src/<file>` holds `text`; returns its manifest path."""
    source = scratch / "src" / file
    source.parent.mkdir(parents=True, exist_ok=True)
    source.write_text(text, encoding="utf-8")
    manifest = scratch / "Cargo.toml"
    manifest.write_text('[package]\nname = "humanitl-catalog"\n', encoding="utf-8")
    return manifest


def main() -> int:
    failures = 0

    def expect(name: str, result: subprocess.CompletedProcess[str], code: int, needle: str = "") -> None:
        nonlocal failures
        if result.returncode != code or needle not in (result.stderr + result.stdout):
            print(f"FAIL {name}: rc={result.returncode} out={result.stdout!r} err={result.stderr!r}")
            failures += 1

    expect(
        "file_crates_only_pass",
        run(graph({"humanitl-catalog": [("humanitl-core", None), ("flate2", None)], "flate2": [("miniz_oxide", None)]})),
        0,
        "free of",
    )
    expect(
        "a_direct_network_crate_fails",
        run(graph({"humanitl-catalog": [("tokio", None)]})),
        1,
        "humanitl-catalog -> tokio",
    )
    expect(
        "a_network_crate_behind_another_fails_with_its_path",
        run(graph({"humanitl-catalog": [("fetcher", None)], "fetcher": [("hyper", None)]})),
        1,
        "humanitl-catalog -> fetcher -> hyper",
    )
    # Zwei Umwege verschiedener Länge, beide erst hinter einem Kind: Eine Suche
    # in die Tiefe nähme den zuletzt gefundenen Zweig zuerst und meldete den
    # längeren Pfad.
    expect(
        "the_path_named_is_the_shortest",
        run(graph({
            "humanitl-catalog": [("x", None), ("y", None)],
            "x": [("tokio", None)],
            "y": [("z", None)],
            "z": [("tokio", None)],
        })),
        1,
        "tokio: humanitl-catalog -> x -> tokio",
    )
    expect(
        "a_dev_dependency_may_reach_the_network",
        run(graph({"humanitl-catalog": [("tokio", "dev")]})),
        0,
    )
    expect(
        "a_build_dependency_may_reach_the_network",
        run(graph({"humanitl-catalog": [("reqwest", "build")]})),
        0,
    )
    expect(
        "a_normal_edge_behind_a_dev_edge_does_not_count",
        run(graph({"humanitl-catalog": [("harness", "dev")], "harness": [("tokio", None)]})),
        0,
    )
    expect(
        "an_offline_crate_missing_from_the_workspace_fails",
        run(graph({"humanitl-core": []})),
        1,
        "not a crate of this workspace",
    )
    no_graph = graph({"humanitl-catalog": []})
    no_graph["resolve"] = None
    expect("metadata_without_a_graph_fails", run(no_graph), 1, "no resolved graph")

    with tempfile.TemporaryDirectory() as scratch:
        manifest = crate_with_source(Path(scratch), "use std::net::TcpStream;\npub fn f() {}\n")
        expect(
            "std_net_in_the_source_fails",
            run(graph({"humanitl-catalog": []}, {"humanitl-catalog": manifest})),
            1,
            "network API in its own source",
        )
    with tempfile.TemporaryDirectory() as scratch:
        manifest = crate_with_source(Path(scratch), "// kein TcpStream, nur ein Kommentar\npub fn f() {}\n")
        expect(
            "a_comment_naming_a_socket_passes",
            run(graph({"humanitl-catalog": []}, {"humanitl-catalog": manifest})),
            0,
        )

    # Kanten, wie cargo sie wirklich schreibt (Review HUM-031): eine Kante, die
    # normal und dev zugleich ist, eine nur für eine Plattform, und ein Crate
    # mit Bindestrich, dessen extern-Name einen Unterstrich trägt.
    expect(
        "a_dependency_both_normal_and_dev_counts",
        run(graph({"humanitl-catalog": [("tokio", [{"kind": None, "target": None}, {"kind": "dev", "target": None}])]})),
        1,
        "humanitl-catalog -> tokio",
    )
    expect(
        "a_platform_specific_normal_dependency_counts",
        run(graph({"humanitl-catalog": [("tokio", [{"kind": None, "target": "cfg(unix)"}])]})),
        1,
        "humanitl-catalog -> tokio",
    )
    expect(
        "a_hyphenated_crate_is_found_by_its_package_name",
        run(graph({"humanitl-catalog": [("hyper-util", None)]})),
        1,
        "humanitl-catalog -> hyper-util",
    )
    with tempfile.TemporaryDirectory() as scratch:
        manifest = crate_with_source(Path(scratch), "pub fn f() { let _ = std::net::TcpStream::connect(\"a:1\"); }\n", "store/mod.rs")
        expect(
            "a_socket_in_a_submodule_fails",
            run(graph({"humanitl-catalog": []}, {"humanitl-catalog": manifest})),
            1,
            "store/mod.rs",
        )
    with tempfile.TemporaryDirectory() as scratch:
        manifest = crate_with_source(Path(scratch), "pub fn f(h: &str) -> bool { h.parse::<std::net::IpAddr>().is_ok() }\n")
        expect(
            "an_ip_address_value_passes",
            run(graph({"humanitl-catalog": []}, {"humanitl-catalog": manifest})),
            0,
        )
    with tempfile.TemporaryDirectory() as scratch:
        manifest = crate_with_source(Path(scratch), "use std::net::*;\npub fn f() { let _ = \"example.com:443\".to_socket_addrs(); }\n")
        expect(
            "a_name_lookup_fails",
            run(graph({"humanitl-catalog": []}, {"humanitl-catalog": manifest})),
            1,
            "network API in its own source",
        )

    if failures:
        print(f"check_offline_test: {failures} failed")
        return 1
    print("check_offline_test: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
