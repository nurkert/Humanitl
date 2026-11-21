#!/usr/bin/env python3
"""Gleicht die Labels des GitHub-Repositorys mit .github/labels.yml ab, ueber `gh`.

Labels sind Code: Die Datei nennt jedes Label mit Farbe, Beschreibung und
frueheren Namen. Dieses Skript bringt das Repository in genau eine Richtung
auf ihren Stand. Ein fehlendes Label wird angelegt, ein vorhandenes bekommt
Farbe und Beschreibung aus der Datei (`gh label create --force`), und ein
Label, das noch einen frueheren Namen (`aliases`) traegt, wird umbenannt;
die Issues, die es tragen, behalten es so. Geloescht wird nie: Dependabot und
GitHub legen eigene Labels an, und ein Label, das in der Datei fehlt, ist kein
Grund, es von Issues zu entfernen.

Die Datei wird hier gelesen, nicht mit PyYAML, damit der Workflow nichts
ausser python3 und gh braucht. Das Format ist bewusst eng (siehe Kopf von
.github/labels.yml); alles ausserhalb ist ein Fehler, nie eine Vermutung.

Aufruf:
  tools/sync_labels.py [--file PFAD] [--check] [--dry-run]

  --check    nur die Datei lesen und pruefen, kein Aufruf von gh
  --dry-run  die vorhandenen Labels lesen, die Aufrufe von gh nur ausgeben

Umgebung: HUMANITL_GH ersetzt den Befehl gh (der Test setzt einen Stub ein);
GH_REPO und GH_TOKEN gehen unveraendert an gh.
"""

from __future__ import annotations

import argparse
import os
import re
import shlex
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_FILE = ROOT / ".github" / "labels.yml"
KEYS = {"name", "color", "description", "aliases"}
COLOR = re.compile(r"^[0-9a-fA-F]{6}$")
# GitHub begrenzt die Beschreibung eines Labels auf 100 Zeichen.
MAX_DESCRIPTION = 100


class LabelFileError(Exception):
    """Die Label-Datei verlaesst das Format oder widerspricht sich."""


@dataclass
class Label:
    name: str
    color: str
    description: str
    aliases: list[str] = field(default_factory=list)


def _scalar(raw: str, where: str) -> str:
    raw = raw.strip()
    if raw.startswith('"'):
        inner = raw[1:-1]
        if len(raw) < 2 or not raw.endswith('"') or '"' in inner or "\\" in inner:
            raise LabelFileError(f"{where}: quoted strings take no escapes and no inner quote")
        return inner
    if raw.startswith("'"):
        if len(raw) < 2 or not raw.endswith("'"):
            raise LabelFileError(f"{where}: unterminated quoted string")
        return raw[1:-1].replace("''", "'")
    if not raw or raw[0] in "[{&*!|>%@`#" or ": " in raw or " #" in raw:
        raise LabelFileError(f"{where}: unsupported value {raw!r}; quote it")
    return raw


def _flow_list(raw: str, where: str) -> list[str]:
    raw = raw.strip()
    if not (raw.startswith("[") and raw.endswith("]")):
        raise LabelFileError(f"{where}: aliases must be a flow list like [a, b]")
    inner = raw[1:-1].strip()
    if not inner:
        return []
    return [_scalar(item, where) for item in inner.split(",")]


def parse(text: str) -> list[Label]:
    """Liest die enge YAML-Teilmenge von .github/labels.yml."""
    entries: list[dict[str, object]] = []
    current: dict[str, object] | None = None
    for number, line in enumerate(text.splitlines(), start=1):
        where = f"line {number}"
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if line.startswith("- "):
            current = {}
            entries.append(current)
            body = line[2:]
        elif line.startswith("  ") and current is not None:
            body = line[2:]
            if body.startswith(" "):
                raise LabelFileError(f"{where}: unexpected indentation")
        else:
            raise LabelFileError(f"{where}: expected '- name: ...' or an indented key")
        key, sep, value = body.partition(":")
        key = key.strip()
        if not sep or key not in KEYS:
            raise LabelFileError(f"{where}: unknown key {key!r}")
        if key in current:
            raise LabelFileError(f"{where}: duplicate key {key!r}")
        current[key] = _flow_list(value, where) if key == "aliases" else _scalar(value, where)

    labels: list[Label] = []
    seen: dict[str, str] = {}
    for index, entry in enumerate(entries, start=1):
        missing = {"name", "color", "description"} - entry.keys()
        if missing:
            raise LabelFileError(f"entry {index}: missing {', '.join(sorted(missing))}")
        label = Label(
            name=str(entry["name"]),
            color=str(entry["color"]).lower(),
            description=str(entry["description"]),
            aliases=list(entry.get("aliases", [])),  # type: ignore[arg-type]
        )
        # Ein Name mit fuehrendem `-` landete als Option auf der Kommandozeile von gh.
        for name in [label.name, *label.aliases]:
            if not name or name.startswith("-"):
                raise LabelFileError(f"{name!r}: a label name must not be empty or start with '-'")
        if not COLOR.match(label.color):
            raise LabelFileError(f"{label.name}: color must be six hex digits without '#'")
        if len(label.description) > MAX_DESCRIPTION:
            raise LabelFileError(f"{label.name}: description longer than {MAX_DESCRIPTION} characters")
        # GitHub vergleicht Label-Namen ohne Ruecksicht auf Gross- und Kleinschreibung.
        for name in [label.name, *label.aliases]:
            folded = name.casefold()
            if folded in seen:
                raise LabelFileError(f"{name!r} appears twice (also in {seen[folded]!r})")
            seen[folded] = label.name
        labels.append(label)
    if not labels:
        raise LabelFileError("no labels in the file")
    return labels


def gh_command() -> list[str]:
    return shlex.split(os.environ.get("HUMANITL_GH", "gh"))


def existing_labels() -> set[str]:
    out = subprocess.run(
        [*gh_command(), "label", "list", "--limit", "1000", "--json", "name", "--jq", ".[].name"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return {line.casefold() for line in out.splitlines() if line}


def plan(labels: list[Label], existing: set[str]) -> list[list[str]]:
    """Die Aufrufe von gh, die das Repository auf den Stand der Datei bringen; nie ein Loeschen."""
    calls: list[list[str]] = []
    for label in labels:
        attrs = ["--color", label.color, "--description", label.description]
        alias = next((a for a in label.aliases if a.casefold() in existing), None)
        if label.name.casefold() not in existing and alias is not None:
            calls.append(["label", "edit", alias, "--name", label.name, *attrs])
        else:
            calls.append(["label", "create", label.name, *attrs, "--force"])
    return calls


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Apply .github/labels.yml through gh.")
    parser.add_argument("--file", type=Path, default=DEFAULT_FILE)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)
    try:
        labels = parse(args.file.read_text(encoding="utf-8"))
    except (OSError, LabelFileError) as err:
        print(f"sync_labels: {args.file}: {err}", file=sys.stderr)
        return 2
    if args.check:
        print(f"sync_labels: {len(labels)} labels, file is valid")
        return 0
    try:
        existing = existing_labels()
    except (OSError, subprocess.CalledProcessError) as err:
        print(f"sync_labels: could not list the labels with gh: {err}", file=sys.stderr)
        return 1
    for call in plan(labels, existing):
        print("gh " + shlex.join(call))
        if args.dry_run:
            continue
        result = subprocess.run([*gh_command(), *call], check=False)
        if result.returncode != 0:
            print(f"sync_labels: gh failed with {result.returncode}", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
