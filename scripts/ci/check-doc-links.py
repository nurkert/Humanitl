#!/usr/bin/env python3
"""Offline link check for the Markdown documentation (HUM-059).

Every relative link in README.md, README.de.md, SECURITY.md and every
Markdown file under docs/ must point at a file or directory that exists, and
a #fragment into a Markdown file must name a heading (GitHub slug rules) or an
explicit <a id=...>/<a name=...> anchor in it. External links (any scheme,
for example https: or mailto:) are not fetched: that would make CI depend on
other people's servers. They are checked by hand before a release.

This replaces `lychee --offline --include-fragments` from the spec: the same
two checks, without a binary that CI would have to download and pin.

Usage:
  scripts/ci/check-doc-links.py               check the repository
  scripts/ci/check-doc-links.py FILE...       check these files only
  scripts/ci/check-doc-links.py --self-test   prove every check fails on a broken case
"""

from __future__ import annotations

import os
import re
import sys
import tempfile
import unicodedata
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[2]
TOP_LEVEL = ("README.md", "README.de.md", "SECURITY.md")

FENCE = re.compile(r"^\s{0,3}(```|~~~)")
INLINE_CODE = re.compile(r"(`+)(.+?)\1")
# [text](target) and ![alt](target); the target ends at whitespace or ")".
MD_LINK = re.compile(r"!?\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^)\s>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
HTML_LINK = re.compile(r"""(?:href|src)\s*=\s*["']([^"']+)["']""")
HEADING = re.compile(r"^\s{0,3}(#{1,6})\s+(.*?)\s*#*\s*$")
EXPLICIT_ANCHOR = re.compile(r"""<a\s+[^>]*(?:id|name)\s*=\s*["']([^"']+)["']""")
SCHEME = re.compile(r"^[a-zA-Z][a-zA-Z0-9+.-]*:")


def strip_code(lines: list[str]) -> list[str]:
    """Blank out fenced code blocks and inline code spans, keep line numbers."""
    out = []
    fenced = None
    for line in lines:
        match = FENCE.match(line)
        if fenced is None and match:
            fenced = match.group(1)
            out.append("")
            continue
        if fenced is not None:
            if line.strip().startswith(fenced):
                fenced = None
            out.append("")
            continue
        out.append(INLINE_CODE.sub(" ", line))
    return out


def slug(text: str) -> str:
    """The anchor GitHub gives a heading."""
    text = re.sub(r"!?\[([^\]]*)\]\([^)]*\)", r"\1", text)  # links keep their text
    text = re.sub(r"<[^>]+>", "", text)  # inline HTML
    text = text.replace("`", "")
    text = text.strip().lower()
    kept = []
    for char in text:
        category = unicodedata.category(char)
        if char in (" ", "-", "_") or category[0] in ("L", "N") or category == "Mn":
            kept.append(char)
    return "".join(kept).replace(" ", "-")


def anchors_of(path: Path, cache: dict[Path, set[str]]) -> set[str]:
    if path in cache:
        return cache[path]
    names: set[str] = set()
    seen: dict[str, int] = {}
    fenced = None
    for line in path.read_text(encoding="utf-8").splitlines():
        match = FENCE.match(line)
        if fenced is None and match:
            fenced = match.group(1)
            continue
        if fenced is not None:
            if line.strip().startswith(fenced):
                fenced = None
            continue
        heading = HEADING.match(line)
        if heading:
            base = slug(heading.group(2))
            count = seen.get(base, 0)
            seen[base] = count + 1
            names.add(base if count == 0 else f"{base}-{count}")
        for anchor in EXPLICIT_ANCHOR.findall(line):
            names.add(anchor)
    cache[path] = names
    return names


def check_file(path: Path, cache: dict[Path, set[str]]) -> list[str]:
    errors = []
    lines = strip_code(path.read_text(encoding="utf-8").splitlines())
    for number, line in enumerate(lines, start=1):
        targets = MD_LINK.findall(line) + HTML_LINK.findall(line)
        for target in targets:
            if SCHEME.match(target) or target.startswith("//"):
                continue
            file_part, _, fragment = target.partition("#")
            file_part = unquote(file_part)
            if file_part:
                resolved = (path.parent / file_part).resolve()
            else:
                resolved = path.resolve()
            where = f"{os.path.relpath(path, ROOT)}:{number}"
            if not resolved.exists():
                errors.append(f"{where}: link target does not exist: {target}")
                continue
            if fragment and resolved.is_file() and resolved.suffix == ".md":
                if unquote(fragment) not in anchors_of(resolved, cache):
                    errors.append(f"{where}: no heading or anchor #{fragment} in "
                                  f"{os.path.relpath(resolved, ROOT)}")
    return errors


def default_files(root: Path) -> list[Path]:
    files = [root / name for name in TOP_LEVEL if (root / name).is_file()]
    files += sorted((root / "docs").rglob("*.md"))
    return files


def run(files: list[Path]) -> int:
    cache: dict[Path, set[str]] = {}
    errors = []
    for path in files:
        errors += check_file(path, cache)
    for error in errors:
        print(f"check-doc-links: {error}", file=sys.stderr)
    if errors:
        print(f"check-doc-links: {len(errors)} broken link(s)", file=sys.stderr)
        return 1
    print(f"check-doc-links: ok ({len(files)} files)")
    return 0


def self_test() -> int:
    """Each case writes one broken link next to a valid page and expects it found."""
    cases = {
        "valid": ("[a](other.md#second-heading) [b](other.md) [c](#local) "
                  "[d](sub/) [e](https://example.com/x) ![i](img.png) "
                  "`[skip](missing.md)` [f](other.md#explicit) [g](other.md#second-heading-1)",
                  None),
        "missing-file": ("[a](nothere.md)", "link target does not exist: nothere.md"),
        "missing-fragment": ("[a](other.md#nope)", "no heading or anchor #nope"),
        "missing-local-fragment": ("[a](#nope)", "no heading or anchor #nope"),
        "html-src": ('<img src="gone.png">', "link target does not exist: gone.png"),
    }
    failures = 0
    with tempfile.TemporaryDirectory() as tmp:
        base = Path(tmp)
        (base / "sub").mkdir()
        (base / "img.png").write_bytes(b"")
        (base / "other.md").write_text(
            "# Other\n\n## Second `heading`\n\n## Second heading\n\n<a id=\"explicit\"></a>\n\n"
            "```\n## not a heading\n```\n", encoding="utf-8")
        for name, (body, want) in cases.items():
            page = base / f"{name}.md"
            page.write_text(f"# Page\n\n## Local\n\n{body}\n", encoding="utf-8")
            errors = check_file(page, {})
            if want is None and errors:
                print(f"self-test: {name}: expected no error, got {errors}", file=sys.stderr)
                failures += 1
            elif want is not None and not any(want in e for e in errors):
                print(f"self-test: {name}: expected '{want}', got {errors}", file=sys.stderr)
                failures += 1
    if failures == 0:
        print(f"check-doc-links: self-test ok ({len(cases)} cases)")
    return 1 if failures else 0


def main(argv: list[str]) -> int:
    if argv[1:] == ["--self-test"]:
        return self_test()
    if len(argv) > 1:
        return run([Path(arg).resolve() for arg in argv[1:]])
    return run(default_files(ROOT))


if __name__ == "__main__":
    sys.exit(main(sys.argv))
