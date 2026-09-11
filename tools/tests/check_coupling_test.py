#!/usr/bin/env python3
"""Tests for tools/check_coupling.py (run: python3 tools/tests/check_coupling_test.py)."""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))

import check_coupling  # noqa: E402  (the path above makes it importable)


def tree(files: dict[str, str]) -> Path:
    """A throwaway source tree without git, so the walk is what runs."""
    root = Path(tempfile.mkdtemp(prefix="coupling-"))
    for name, text in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    return root


def check(root: Path, baseline: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(ROOT / "tools" / "check_coupling.py"),
         "--root", str(root), "--baseline", str(baseline)],
        capture_output=True, text=True, check=False,
    )


def update(root: Path, baseline: Path) -> None:
    subprocess.run(
        [sys.executable, str(ROOT / "tools" / "check_coupling.py"),
         "--root", str(root), "--baseline", str(baseline), "--update"],
        capture_output=True, text=True, check=True,
    )


FAILURES: list[str] = []


def expect(condition: bool, what: str) -> None:
    if not condition:
        FAILURES.append(what)


def test_bwrap_counts_outside_the_adapter_only() -> None:
    root = tree({
        "daemon/crates/sandbox/src/bwrap.rs": "// bwrap bwrap bwrap\n",
        "daemon/bin/humanitl-shim/src/main.rs": "// bwrap\n",
        "daemon/crates/ipc/src/sandbox.rs": (
            "// Bwrap and bwrap\nlet backend: BwrapBackend = sandbox::bwrap_args::build();\n"
        ),
    })
    counts = check_coupling.measure(root)
    expect(counts["bwrap"] == {"daemon/crates/ipc/src/sandbox.rs": 4},
           f"bwrap is counted outside the adapter, in any case, inside identifiers too: "
           f"{counts['bwrap']}")


def test_a_proxy_use_list_counts_every_name_across_lines() -> None:
    root = tree({
        "daemon/crates/ipc/src/x.rs": (
            "use humanitl_proxy::{\n    HoldQueue,\n    registry::FlowRecord,\n"
            "    DomainSink as _,\n};\nfn f() { humanitl_proxy::roots_from_pem(); }\n"
        ),
        "daemon/crates/proxy/src/lib.rs": "use humanitl_proxy::Anything;\n",
    })
    counts = check_coupling.measure(root)
    expect(counts["proxy_crate"] == {"daemon/crates/ipc/src/x.rs": 4},
           f"three names in the list and one plain path make four: {counts['proxy_crate']}")


def test_a_nested_use_list_counts_its_leaves() -> None:
    root = tree({
        "daemon/crates/ipc/src/n.rs": "use humanitl_proxy::{ca::{CaStore, LeafCache}, HoldQueue};\n",
    })
    counts = check_coupling.measure(root)
    expect(counts["proxy_crate"] == {"daemon/crates/ipc/src/n.rs": 3},
           f"the prefix `ca::` is no item, its two leaves and HoldQueue are: {counts['proxy_crate']}")


def test_comment_lines_are_no_proxy_dependency_but_still_name_bwrap() -> None:
    root = tree({
        "daemon/crates/sandbox/src/x.rs": "",
        "daemon/crates/ipc/src/c.rs": (
            "//! See `humanitl_proxy::ca::ENV_KIT` and `hyper::Request`.\n"
            "/// Built by `humanitl_proxy::meta`, run under bwrap.\n"
            "use humanitl_proxy::HoldQueue;\n"
        ),
    })
    counts = check_coupling.measure(root)
    expect(counts["proxy_crate"] == {"daemon/crates/ipc/src/c.rs": 1},
           f"only the `use` line counts for the crate: {counts['proxy_crate']}")
    expect(counts["proxy_engine"] == {},
           f"an engine path in a comment is no dependency: {counts['proxy_engine']}")
    expect(counts["bwrap"] == {"daemon/crates/ipc/src/c.rs": 1},
           f"prose naming the backend still counts: {counts['bwrap']}")


def test_an_alias_of_the_proxy_crate_is_counted() -> None:
    root = tree({
        "daemon/crates/ipc/src/a.rs": "use humanitl_proxy as proxy;\nfn f(_: proxy::HoldQueue) {}\n",
        "daemon/crates/ipc/src/b.rs": "use humanitl_proxy;\n",
        "daemon/crates/ipc/src/c.rs": "use humanitl_proxy::{self as proxy};\n",
        "daemon/crates/ipc/src/d.rs": "use humanitl_proxy::ca as c;\n",
        "daemon/crates/ipc/src/e.rs": "use humanitl_proxy::ca::*;\n",
        "daemon/crates/ipc/src/f.rs": (
            "pub use humanitl_proxy::DEFAULT_PORTS as DEFAULT_DISCOVER_PORTS;\n"
            "use humanitl_proxy::{HoldQueue as Queue};\n"
        ),
    })
    root_spellings = {
        "g.rs": "use ::humanitl_proxy as proxy;\n",
        "i.rs": "use {humanitl_proxy as proxy};\n",
        "j.rs": "use ::humanitl_proxy::*;\n",
        "l.rs": "use {humanitl_proxy};\n",
        "m.rs": "use humanitl_proxy::ca;\n",
        "n.rs": "use humanitl_proxy::ca::{self, CaStore};\n",
    }
    for name, text in root_spellings.items():
        (root / "daemon/crates/ipc/src" / name).write_text(text, encoding="utf-8")
    counts = check_coupling.measure(root)
    expected = {f"daemon/crates/ipc/src/{name}.rs": 1 for name in "abcdegijlmn"}
    expect(counts["proxy_alias"] == expected,
           f"every way to hide later uses is counted, renaming one item is not: "
           f"{counts['proxy_alias']}")


def test_a_list_behind_a_module_path_counts_its_names() -> None:
    root = tree({
        "daemon/crates/ipc/src/p.rs": (
            "use humanitl_proxy::ca::{CaStore, LeafCache};\n"
            "use humanitl_proxy:: {\n    HoldQueue,\n    FlowRegistry,\n};\n"
        ),
    })
    counts = check_coupling.measure(root)
    expect(counts["proxy_crate"] == {"daemon/crates/ipc/src/p.rs": 4},
           f"two names behind `ca::` and two behind whitespace: {counts['proxy_crate']}")


def test_code_behind_or_beside_a_block_comment_still_counts() -> None:
    root = tree({
        "daemon/crates/ipc/src/k.rs": (
            "/* note */ use humanitl_proxy::HoldQueue;\n"
            "/*\n * humanitl_proxy::NotCode and hyper::NotCode\n */\n"
            "let glob = \"src/*\"; use humanitl_proxy::FlowRegistry; let tail = \"*/\";\n"
            "let quote = '\"'; use humanitl_proxy::RulesStore; let other = \"x\";\n"
            "let raw = r#\"a \" /* \"#; use humanitl_proxy::AfterRaw; let z = \"*/\";\n"
            "fn f<'a>(_: &'a str) {}\n"
        ),
        # A parser that takes the quote in the char literal for a string would
        # close it at the next quote and read `src/*` as an opening comment.
        "daemon/crates/ipc/src/q.rs": (
            "let quote = '\"'; let glob = \"src/*\"; use humanitl_proxy::RulesStore; "
            "let tail = \"*/\";\n"
        ),
    })
    counts = check_coupling.measure(root)
    expect(counts["proxy_crate"] == {"daemon/crates/ipc/src/k.rs": 4,
                                     "daemon/crates/ipc/src/q.rs": 1},
           f"every use beside comments and literals counts, none in the block comment: "
           f"{counts['proxy_crate']}")
    expect(counts["proxy_engine"] == {},
           f"an engine path inside a block comment is no dependency: {counts['proxy_engine']}")


def test_an_alias_of_an_engine_crate_is_counted() -> None:
    root = tree({
        "daemon/crates/ipc/src/h.rs": "use hyper as h;\nextern crate rustls;\n",
        "daemon/crates/ipc/src/w.rs": (
            "use hyper /* note */ ::Request;\nuse {rcgen, webpki_roots};\n"
            "use humanitl_proxy /* note */ ::HoldQueue;\n"
        ),
    })
    counts = check_coupling.measure(root)
    expect(counts["proxy_engine"] == {"daemon/crates/ipc/src/h.rs": 2,
                                      "daemon/crates/ipc/src/w.rs": 3},
           f"an aliased, bare, grouped or spaced engine crate counts: {counts['proxy_engine']}")
    expect(counts["proxy_crate"] == {"daemon/crates/ipc/src/w.rs": 1},
           f"a comment before `::` hides no proxy item: {counts['proxy_crate']}")


def test_an_untracked_file_in_a_git_tree_counts() -> None:
    root = tree({"daemon/crates/ipc/src/a.rs": "// bwrap\n"})
    git = ["git", "-C", str(root), "-c", "user.name=t", "-c", "user.email=t@t",
           "-c", "commit.gpgsign=false"]
    subprocess.run([*git, "init", "-q"], check=True)
    subprocess.run([*git, "add", "."], check=True)
    subprocess.run([*git, "commit", "-q", "-m", "base"], check=True)
    baseline = root / "baseline.toml"
    update(root, baseline)
    (root / "app/lib").mkdir(parents=True)
    (root / "app/lib/new.dart").write_text("// bwrap\n", encoding="utf-8")
    (root / ".gitignore").write_text("ignored.rs\n", encoding="utf-8")
    (root / "ignored.rs").write_text("// bwrap\n", encoding="utf-8")
    result = check(root, baseline)
    expect(result.returncode == 1 and "app/lib/new.dart: 0 -> 1" in result.stderr
           and "ignored.rs" not in result.stderr,
           f"git lists the untracked file and skips the ignored one: "
           f"rc={result.returncode} {result.stderr}")


def test_an_engine_type_outside_the_proxy_is_counted() -> None:
    root = tree({
        "daemon/crates/ipc/src/y.rs": "fn g(_: hyper::Request<()>, _: rustls::ClientConfig) {}\n",
        "daemon/crates/proxy/src/tls.rs": "use rustls::ServerConfig;\n",
    })
    counts = check_coupling.measure(root)
    expect(counts["proxy_engine"] == {"daemon/crates/ipc/src/y.rs": 2},
           f"engine paths count outside the proxy crate only: {counts['proxy_engine']}")


def test_the_ratchet_passes_when_nothing_moved() -> None:
    root = tree({"daemon/crates/ipc/src/a.rs": "// bwrap\n"})
    baseline = root / "baseline.toml"
    update(root, baseline)
    result = check(root, baseline)
    expect(result.returncode == 0, f"an unchanged tree passes: {result.stderr}")


def test_a_rise_fails() -> None:
    root = tree({"daemon/crates/ipc/src/a.rs": "// bwrap\n"})
    baseline = root / "baseline.toml"
    update(root, baseline)
    (root / "daemon/crates/ipc/src/a.rs").write_text("// bwrap bwrap\n", encoding="utf-8")
    result = check(root, baseline)
    expect(result.returncode == 1 and "coupling grew" in result.stderr,
           f"one more mention fails: rc={result.returncode} {result.stderr}")


def test_a_new_file_fails() -> None:
    root = tree({"daemon/crates/ipc/src/a.rs": "// bwrap\n"})
    baseline = root / "baseline.toml"
    update(root, baseline)
    (root / "app/lib/b.dart").parent.mkdir(parents=True, exist_ok=True)
    (root / "app/lib/b.dart").write_text("// bwrap\n", encoding="utf-8")
    result = check(root, baseline)
    expect(result.returncode == 1 and "app/lib/b.dart: 0 -> 1" in result.stderr,
           f"a new file with the name fails: rc={result.returncode} {result.stderr}")


def test_a_fall_asks_for_the_baseline_to_follow() -> None:
    root = tree({"daemon/crates/ipc/src/a.rs": "// bwrap bwrap\n"})
    baseline = root / "baseline.toml"
    update(root, baseline)
    (root / "daemon/crates/ipc/src/a.rs").write_text("// bwrap\n", encoding="utf-8")
    result = check(root, baseline)
    expect(result.returncode == 1 and "--update" in result.stderr,
           f"a fall fails until the baseline follows, so the slack cannot be spent again: "
           f"rc={result.returncode} {result.stderr}")
    update(root, baseline)
    expect(check(root, baseline).returncode == 0, "after --update the tree passes again")


def main() -> int:
    for name, test in sorted(globals().items()):
        if name.startswith("test_") and callable(test):
            test()
    for failure in FAILURES:
        print(f"FAIL: {failure}", file=sys.stderr)
    print(f"check_coupling_test: {'FAILED' if FAILURES else 'ok'}")
    return 1 if FAILURES else 0


if __name__ == "__main__":
    sys.exit(main())
