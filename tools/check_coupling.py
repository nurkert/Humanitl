#!/usr/bin/env python3
"""Keep adapter-specific names inside their adapters: a ratchet.

Humanitl draws its adapters as the outer ring (ADR-0015): bubblewrap behind the
`SandboxBackend` port, the hyper/rustls engine inside the proxy crate. Where
their names appear outside that ring, the code is coupled to one adapter, and a
second one -- microsandbox as a sandbox backend, an established proxy engine --
gets harder to add. That coupling exists today and is measured; this check
freezes it. A count may fall, never rise, and no new file may start using one
of the names. When a count falls, the baseline has to follow
(`--update`), otherwise the slack could be spent again unnoticed.

Four rules:

* `bwrap` -- the string, in any case, outside the sandbox crate and the shim,
  inside identifiers (`BwrapBackend`) and comments included.
* `proxy_engine` -- a path into hyper, rustls, rcgen or their helpers outside
  the proxy crate. Zero today; it stays zero.
* `proxy_crate` -- every item of `humanitl_proxy` referenced outside the proxy
  crate, counted per name, including every name inside a `use` list. The crate
  mixes the engine with the application logic (hold queue, registry, rules
  store); until that is cut, nothing new may depend on it.
* `proxy_alias` -- `use humanitl_proxy as p;`, `use humanitl_proxy::ca as c;`,
  `{self as p}`, `use humanitl_proxy::ca;`, a glob and a bare
  `use humanitl_proxy;`. Each would let later uses pass uncounted; the count
  per file freezes the few that exist.

The proxy rules skip `//` comment lines: a doc comment that points at
`humanitl_proxy::ca::ENV_KIT` depends on nothing.

Usage:
  python3 tools/check_coupling.py            # check against the baseline
  python3 tools/check_coupling.py --update   # write the current counts
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Rule:
    """One kind of coupling: what is counted, and where it is at home."""

    pattern: re.Pattern[str]
    allowed: tuple[str, ...]
    why: str


RULES: dict[str, Rule] = {
    # A substring, not a word: the tightest coupling is `BwrapBackend` held as a
    # concrete type and `bwrap_args`, and `\b` would see neither.
    "bwrap": Rule(
        re.compile(r"bwrap", re.IGNORECASE),
        ("daemon/crates/sandbox/", "daemon/bin/humanitl-shim/"),
        "bubblewrap is one SandboxBackend; outside it, name the neutral concept",
    ),
    "proxy_engine": Rule(
        re.compile(
            r"\b(?:extern\s+crate\s+)?"
            r"(?:hyper|hyper_util|http_body_util|rustls|tokio_rustls|rcgen|webpki_roots)"
            r"(?:\s*::|\s+as\b|\s*[,;}])"
        ),
        ("daemon/crates/proxy/",),
        "the protocol engine belongs to the proxy crate; nothing outside uses its types",
    ),
    "proxy_crate": Rule(
        re.compile(r"\bhumanitl_proxy\s*::"),
        ("daemon/crates/proxy/",),
        "the proxy crate mixes engine and application; do not widen what depends on it",
    ),
    # Every `use` statement that names the crate, however it is spelled:
    # `use ::humanitl_proxy as p;` and `use {humanitl_proxy as p};` are Rust too.
    "proxy_alias": Rule(
        re.compile(r"\b(?:use|extern\s+crate)\b[^;]*?\bhumanitl_proxy\b[^;]*"),
        ("daemon/crates/proxy/",),
        "an alias, a glob or a module import hides later uses from the proxy_crate count",
    ),
}

# Where a `use` list opens behind the crate name, perhaps after a module path.
LIST_OPENING = re.compile(r"(?:\s*\w+\s*::)*\s*\{")

# Inside a `use humanitl_proxy...` statement, what lets later code name the
# crate's items without writing `humanitl_proxy::`: a glob, a lower-case name
# renamed with `as` (a module, `self`, the crate itself), or a lower-case leaf
# (`use humanitl_proxy::ca;`, `ca::{self, CaStore}`), after which `ca::X` goes
# uncounted. A lower-case leaf may also be a function; it is flagged all the
# same, since the import is new coupling either way. Renaming a type or a
# constant stays allowed: it names one item, just as the import does.
HIDING = re.compile(
    r"\*|\b[a-z_][a-z0-9_]*\s+as\b|(?:::|[{,])\s*[a-z_][a-z0-9_]*\s*(?=[,;}]|$)"
)

EXTENSIONS = (".rs", ".dart", ".proto")

# Directories a walk never enters. `git ls-files` is used where it works; the
# walk is the fallback for a tree without git, which is what the tests build.
PRUNED = {".git", "target", "build", ".dart_tool", "generated", "node_modules"}


def tracked_files(root: Path) -> list[str]:
    """The source files under [root], relative, with forward slashes.

    Untracked files count as well, unless ignored: `make check` runs before the
    commit, and a new file that names `bwrap` must fail there, not first in CI.
    """
    try:
        listed = subprocess.run(
            ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
            cwd=root,
            capture_output=True,
            check=True,
        ).stdout.decode("utf-8", "replace")
        names = [name for name in listed.split("\0") if name]
    except (subprocess.CalledProcessError, FileNotFoundError):
        names = []
        for directory, subdirs, files in os.walk(root):
            subdirs[:] = [name for name in subdirs if name not in PRUNED]
            for name in files:
                full = Path(directory, name)
                names.append(full.relative_to(root).as_posix())
    return sorted(name for name in names if name.endswith(EXTENSIONS))


def proxy_items(text: str) -> int:
    """How many items of `humanitl_proxy` a file names.

    A plain path (`humanitl_proxy::HoldQueue`) is one item; a `use` list
    (`humanitl_proxy::{A, b::C, D as E}`) counts every name in it, across
    lines, so that adding a name to an existing list is not free.
    """
    count = 0
    for match in RULES["proxy_crate"].pattern.finditer(text):
        rest = text[match.end():]
        # The list may sit behind a module path and whitespace:
        # `humanitl_proxy::ca::{CaStore, LeafCache}` names two items, not one.
        opening = LIST_OPENING.match(rest)
        if opening is None:
            count += 1
            continue
        rest = rest[opening.end() - 1:]
        depth = 0
        inner = []
        for char in rest:
            if char == "{":
                depth += 1
                if depth == 1:
                    continue
            elif char == "}":
                depth -= 1
                if depth == 0:
                    break
            inner.append(char)
        body = "".join(inner)
        # A nested list (`a::{B, C}`) splits into the prefix `a::` and its
        # leaves; the prefix names no item, the leaves do.
        names = [
            part
            for part in re.split(r"[,{}]", body)
            if part.strip() and not part.strip().endswith("::")
        ]
        count += max(1, len(names))
    return count


def proxy_aliases(code: str) -> int:
    """How many `use` statements of `humanitl_proxy` could hide later uses."""
    count = 0
    for match in RULES["proxy_alias"].pattern.finditer(code):
        statement = match.group(0)
        tail = statement[statement.index("humanitl_proxy"):]
        bare = tail.strip(" \t\r\n{}") == "humanitl_proxy"
        if statement.startswith("extern") or bare or HIDING.search(tail):
            count += 1
    return count


# The places where Rust code stops being code: comments, and the literals in
# which `//` or `/*` are only text. Raw strings (`r#"..."#`) close with their
# own number of hashes; a quote is a char literal or the start of a lifetime.
RUST_SPECIAL = re.compile(r"""//|/\*|(?<!\w)[bc]?r\#*"|(?<!\w)[bc]"|"|'""")
RUST_CHAR = re.compile(r"'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]{1,6}\}|.)|[^'\\\n])'")
BLOCK_MARK = re.compile(r"/\*|\*/")
COMMENT_LINE = re.compile(r"^\s*//")


def rust_code(text: str) -> str:
    """[text] with every Rust comment removed, string and char literals kept.

    The proxy rules measure code that depends on the crate; a doc comment that
    points a reader at `humanitl_proxy::ca::ENV_KIT` depends on nothing. A line
    filter would not do: `/* note */ use humanitl_proxy::X;` carries code behind
    the comment, and `"src/*"` opens no comment at all.
    """
    out: list[str] = []
    i, end_of_text = 0, len(text)
    while (special := RUST_SPECIAL.search(text, i)) is not None:
        out.append(text[i:special.start()])
        token, i = special.group(0), special.start()
        if token == "//":
            newline = text.find("\n", i)
            i = end_of_text if newline < 0 else newline
        elif token == "/*":
            depth, i = 1, i + 2
            while depth:
                mark = BLOCK_MARK.search(text, i)
                if mark is None:
                    i = end_of_text
                    break
                depth += 1 if mark.group(0) == "/*" else -1
                i = mark.end()
            out.append(" ")
        elif token.endswith('"') and "r" in token:
            closing = '"' + token[token.index("r") + 1:-1]
            found = text.find(closing, special.end())
            end = end_of_text if found < 0 else found + len(closing)
            out.append(text[i:end])
            i = end
        elif token.endswith('"'):
            end = special.end()
            while end < end_of_text and text[end] != '"':
                end += 2 if text[end] == "\\" else 1
            out.append(text[i:end + 1])
            i = end + 1
        else:
            literal = RUST_CHAR.match(text, i)
            end = literal.end() if literal else i + 1
            out.append(text[i:end])
            i = end
    out.append(text[i:])
    return "".join(out)


def code_of(name: str, text: str) -> str:
    """[text] without comments: fully for Rust, by `//` line for the rest.

    The `bwrap` rule does not use this; it counts comments, because prose that
    names one backend is the kind of knowledge a second backend has to hunt down.
    """
    if name.endswith(".rs"):
        return rust_code(text)
    return "\n".join(line for line in text.splitlines() if not COMMENT_LINE.match(line))


def measure(root: Path) -> dict[str, dict[str, int]]:
    """The current count of every rule, per file, outside the adapter."""
    counts: dict[str, dict[str, int]] = {name: {} for name in RULES}
    for name in tracked_files(root):
        path = root / name
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        code = code_of(name, text)
        for rule_name, rule in RULES.items():
            if name.startswith(rule.allowed):
                continue
            if rule_name == "bwrap":
                found = len(rule.pattern.findall(text))
            elif rule_name == "proxy_crate":
                found = proxy_items(code)
            elif rule_name == "proxy_alias":
                found = proxy_aliases(code)
            else:
                found = len(rule.pattern.findall(code))
            if found:
                counts[rule_name][name] = found
    return counts


def load_baseline(path: Path) -> dict[str, dict[str, int]]:
    """The committed counts; a missing section means zero everywhere."""
    with path.open("rb") as handle:
        doc = tomllib.load(handle)
    return {name: dict(doc.get(name, {})) for name in RULES}


def write_baseline(path: Path, counts: dict[str, dict[str, int]]) -> None:
    """Writes [counts] as TOML, sorted, so a diff shows exactly what moved."""
    lines = [
        "# Written by `python3 tools/check_coupling.py --update`; checked by",
        "# `make deps-lint`. Each number is how often an adapter-specific name",
        "# appears in that file outside its adapter. It may fall, never rise.",
        "",
    ]
    for rule_name in RULES:
        lines.append(f"[{rule_name}]")
        for file_name, count in sorted(counts[rule_name].items()):
            lines.append(f'"{file_name}" = {count}')
        lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def compare(
    current: dict[str, dict[str, int]], baseline: dict[str, dict[str, int]]
) -> tuple[list[str], list[str]]:
    """Rises (the check fails) and falls (the baseline has to follow)."""
    rises: list[str] = []
    falls: list[str] = []
    for rule_name, rule in RULES.items():
        now = current.get(rule_name, {})
        before = baseline.get(rule_name, {})
        for file_name in sorted(set(now) | set(before)):
            new, old = now.get(file_name, 0), before.get(file_name, 0)
            if new > old:
                rises.append(
                    f"{rule_name}: {file_name}: {old} -> {new} ({rule.why})"
                )
            elif new < old:
                falls.append(f"{rule_name}: {file_name}: {old} -> {new}")
    return rises, falls


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", default=Path(__file__).resolve().parents[1], type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--update", action="store_true")
    args = parser.parse_args(argv)
    baseline_path = args.baseline or args.root / "tools" / "coupling-baseline.toml"

    current = measure(args.root)
    if args.update:
        write_baseline(baseline_path, current)
        total = sum(sum(files.values()) for files in current.values())
        print(f"check_coupling: baseline written, {total} mentions")
        return 0
    if not baseline_path.exists():
        print(f"check_coupling: no baseline at {baseline_path}; run --update", file=sys.stderr)
        return 1

    rises, falls = compare(current, load_baseline(baseline_path))
    for line in rises:
        print(f"coupling grew: {line}", file=sys.stderr)
    for line in falls:
        print(
            f"coupling fell (good): {line}; run `python3 tools/check_coupling.py --update`",
            file=sys.stderr,
        )
    return 1 if rises or falls else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
