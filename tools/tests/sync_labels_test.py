#!/usr/bin/env python3
"""Tests fuer tools/sync_labels.py (Aufruf: python3 tools/tests/sync_labels_test.py).

Ein Stub ersetzt gh (HUMANITL_GH): Er meldet eine feste Liste vorhandener
Labels und schreibt jeden weiteren Aufruf mit. Geprueft wird, dass die echte
Datei gueltig ist, dass fehlende Labels angelegt und Altnamen umbenannt
werden, und vor allem, dass nie ein Label geloescht wird.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "sync_labels.py"
LABELS = ROOT / ".github" / "labels.yml"

STUB = textwrap.dedent(
    """\
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ "$1 $2" == "label list" ]]; then
      cat "$STUB_DIR/existing"
      exit 0
    fi
    printf '%s\\0' "$@" >> "$STUB_DIR/calls"
    printf '\\n' >> "$STUB_DIR/calls"
    """
)


def run(file: Path, existing: list[str], *flags: str) -> tuple[subprocess.CompletedProcess[str], list[list[str]]]:
    with tempfile.TemporaryDirectory() as tmp:
        stub = Path(tmp) / "gh"
        stub.write_text(STUB)
        stub.chmod(0o755)
        (Path(tmp) / "existing").write_text("".join(f"{n}\n" for n in existing))
        env = dict(os.environ, HUMANITL_GH=str(stub), STUB_DIR=tmp)
        proc = subprocess.run(
            [sys.executable, str(SCRIPT), "--file", str(file), *flags],
            capture_output=True,
            text=True,
            env=env,
            check=False,
        )
        calls_path = Path(tmp) / "calls"
        raw = calls_path.read_text() if calls_path.exists() else ""
    calls = [line.split("\0")[:-1] for line in raw.split("\n") if line]
    return proc, calls


def write(text: str) -> Path:
    handle = tempfile.NamedTemporaryFile("w", suffix=".yml", delete=False)
    handle.write(textwrap.dedent(text))
    handle.close()
    return Path(handle.name)


class SyncLabelsTest(unittest.TestCase):
    def test_repository_file_is_valid(self) -> None:
        proc, calls = run(LABELS, [], "--check")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(calls, [])

    def test_repository_file_has_every_group(self) -> None:
        text = LABELS.read_text()
        for name in [
            "type:feature", "type:bug", "type:docs", "type:chore", "type:security",
            "area:daemon", "area:proxy", "area:sandbox", "area:cli", "area:app",
            "area:packaging", "area:ci", "area:docs", "area:website",
            "size:S", "size:M", "size:L",
            "status:blocked", "status:needs-decision", "status:needs-info",
            "status:owner-walkthrough",
        ]:
            self.assertIn(f'name: "{name}"', text)
        self.assertRegex(text, r'name: "sprint:\d+"')

    def test_creates_updates_and_never_deletes(self) -> None:
        file = write(
            """\
            # comment
            - name: "type:bug"
              color: "D73A4A"
              description: "Broken"
              aliases: [bug]
            - name: "area:cli"
              color: "5319e7"
              description: "CLI"
            """
        )
        # dependencies kommt von Dependabot und fehlt in der Datei.
        proc, calls = run(file, ["bug", "area:cli", "dependencies"])
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(
            calls,
            [
                ["label", "edit", "bug", "--name", "type:bug", "--color", "d73a4a", "--description", "Broken"],
                ["label", "create", "area:cli", "--color", "5319e7", "--description", "CLI", "--force"],
            ],
        )
        for call in calls:
            self.assertNotIn("delete", call)

    def test_existing_canonical_name_wins_over_alias(self) -> None:
        file = write(
            """\
            - name: "type:bug"
              color: "d73a4a"
              description: "Broken"
              aliases: [bug]
            """
        )
        proc, calls = run(file, ["bug", "Type:Bug"])
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(calls[0][:3], ["label", "create", "type:bug"])

    def test_dry_run_calls_nothing(self) -> None:
        proc, calls = run(LABELS, [], "--dry-run")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(calls, [])
        self.assertIn("gh label create type:bug", proc.stdout)

    def test_rejects_bad_files(self) -> None:
        bad = {
            "color": '- name: "a"\n  color: "#ffffff"\n  description: "x"\n',
            "missing": '- name: "a"\n  color: "ffffff"\n',
            "unknown key": '- name: "a"\n  colour: "ffffff"\n  description: "x"\n',
            "duplicate": '- name: "a"\n  color: "ffffff"\n  description: "x"\n'
            '- name: "A"\n  color: "ffffff"\n  description: "y"\n',
            "alias clash": '- name: "a"\n  color: "ffffff"\n  description: "x"\n  aliases: [b]\n'
            '- name: "b"\n  color: "ffffff"\n  description: "y"\n',
            "long": '- name: "a"\n  color: "ffffff"\n  description: "' + "x" * 101 + '"\n',
            "block list": '- name: "a"\n  color: "ffffff"\n  description: "x"\n  aliases:\n    - b\n',
            "empty": "# nothing\n",
            "leading dash": '- name: "--force"\n  color: "ffffff"\n  description: "x"\n',
            "leading dash alias": '- name: "a"\n  color: "ffffff"\n  description: "x"\n  aliases: [-b]\n',
        }
        for case, text in bad.items():
            with self.subTest(case=case):
                proc, calls = run(write(text), ["a"])
                self.assertEqual(proc.returncode, 2, proc.stdout)
                self.assertEqual(calls, [])


if __name__ == "__main__":
    unittest.main()
