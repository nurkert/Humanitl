#!/usr/bin/env python3
"""Zählt die Token des Agent-Briefings (HUM-071, ADR-0014).

Das Briefing ist der einzige Text, den Humanitl dem Agenten mitgibt, und er
steht in jedem Kontextfenster jeder Sitzung. Jedes Token darin fehlt dem
Agenten für seine eigentliche Arbeit; ADR-0014 nennt deshalb etwa 150 Token
als Ziel und `backlog/sprint-3.md` (HUM-071) 160 als Grenze; gemessen wurde
danach, und die geltende Grenze steht unten in `BUDGET`.

Gezählt wird nicht die Vorlage, sondern das, was in der Sandbox landet: die
Kommentare sind entfernt, der Block zum Ask-Modus ist eingesetzt und die
Platzhalter sind ersetzt. Gerechnet wird mit dem ungünstigsten Fall — der
längere der beiden Ask-Blöcke, ein LLM-Endpunkt mit Host und Port, eine
vierstellige Frist —, denn eine Grenze, die nur für kurze Werte gilt, ist
keine.

Maßgeblich ist `o200k_base`, die Kodierung der aktuellen GPT-Modelle.
`cl100k_base` wird mitgezählt und ausgegeben, aber nicht geprüft: sie zerlegt
deutsche Wörter deutlich feiner, und ein Budget an ihr auszurichten hieße, den
deutschen Text gegenüber dem englischen zu verstümmeln.

Ohne `tiktoken` sagt das Skript das und endet mit 0, wie `make check` es auch
bei fehlendem rustfmt oder clippy tut. Ein Werkzeug, das nicht installiert ist,
ist eine Aussage über die Maschine, nicht über den Text.

    python3 tools/briefing-tokens.py            # prüfen
    python3 tools/briefing-tokens.py --print    # den gerenderten Text zeigen

Installation: `pip install tiktoken` (lädt beim ersten Lauf die BPE-Tabelle).
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# Beide Sprachen hoechstens so viele Token. Dieselbe Zahl steht als
# TOKEN_BUDGET in daemon/crates/sandbox/src/agent/briefing.rs, und dort steht
# auch, warum es 185 sind und nicht die 160 aus backlog/sprint-3.md: der
# Meta-Endpunkt kam nach der Schaetzung dazu, und deutscher Text kostet in
# jedem BPE-Tokenisierer rund 13 Prozent mehr als derselbe englische.
BUDGET = 185

# Die Kodierung, an der geprueft wird, und die, die nur berichtet wird.
ENFORCED = "o200k_base"
REPORTED = ("o200k_base", "cl100k_base")

ROOT = Path(__file__).resolve().parent.parent
TEMPLATES = {
    "en": ROOT / "agents" / "opencode" / "briefing.en.md",
    "de": ROOT / "agents" / "opencode" / "briefing.de.md",
}

VARIANT_MARKER = "<!-- ask_mode:"
COMMENT = re.compile(r"<!--.*?-->", re.DOTALL)

# Der unguenstigste Fall, den die Konfiguration hergibt: ein Endpunkt mit
# einem langen Namen und Port und eine vierstellige Frist. Kuerzere Werte
# koennen den Text nur kleiner machen.
WORST_LLM_HOST = "ollama.services.example.internal:11434"
WORST_TIMEOUT = "3600"


def split_variants(template: str) -> tuple[str, dict[str, str]]:
    """Trennt den Rumpf von den Bloecken unter `<!-- ask_mode: ... -->`."""
    lines = template.splitlines()
    start = next(
        (i for i, line in enumerate(lines) if line.lstrip().startswith(VARIANT_MARKER)),
        len(lines),
    )
    body = "\n".join(lines[:start])
    variants: dict[str, str] = {}
    name: str | None = None
    for line in lines[start:]:
        stripped = line.strip()
        if stripped.startswith(VARIANT_MARKER) and stripped.endswith("-->"):
            name = stripped[len(VARIANT_MARKER) : -len("-->")].strip()
            variants[name] = ""
            continue
        if name is not None:
            variants[name] = (variants[name] + " " + stripped).strip()
    return body, variants


def render(template: str, variant: str) -> str:
    """Der Text, wie er in der Sandbox liegt, mit den ungueltigsten Werten."""
    body, variants = split_variants(template)
    if variant not in variants:
        raise SystemExit(f"briefing-tokens: no block for ask_mode {variant!r}")
    text = COMMENT.sub("", body)
    text = text.replace("{ask_mode}", variants[variant])
    text = text.replace("{timeout}", WORST_TIMEOUT)
    text = text.replace("{llm_host}", WORST_LLM_HOST)
    # Wie `tidy` im Renderer: keine doppelten Leerzeilen, genau ein Zeilenende.
    out: list[str] = []
    for line in text.splitlines():
        line = line.rstrip()
        if not line and (not out or not out[-1]):
            continue
        out.append(line)
    while out and not out[-1]:
        out.pop()
    return "\n".join(out) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description="Count the tokens of the agent briefing")
    parser.add_argument(
        "--print",
        dest="show",
        action="store_true",
        help="print the rendered briefing instead of only the counts",
    )
    args = parser.parse_args()

    try:
        import tiktoken
    except ImportError:
        print(
            "SKIP briefing-tokens: tiktoken is not installed (pip install tiktoken)",
            file=sys.stderr,
        )
        return 0

    encodings = {name: tiktoken.get_encoding(name) for name in REPORTED}
    failed = False
    for language, path in sorted(TEMPLATES.items()):
        template = path.read_text(encoding="utf-8")
        for variant in ("ui", "none"):
            text = render(template, variant)
            if args.show:
                print(f"--- {language} / ask_mode={variant} ---")
                print(text)
            counts = {name: len(enc.encode(text)) for name, enc in encodings.items()}
            report = " ".join(f"{name}={count}" for name, count in counts.items())
            status = "ok" if counts[ENFORCED] <= BUDGET else "OVER BUDGET"
            print(f"{language} ask_mode={variant:5} {report} chars={len(text)} {status}")
            if counts[ENFORCED] > BUDGET:
                failed = True

    if failed:
        print(
            f"briefing-tokens: over the budget of {BUDGET} tokens in {ENFORCED}; "
            "shorten the template, do not raise the budget",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
