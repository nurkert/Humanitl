#!/usr/bin/env bash
# Gleicht das Skelett in `models.json` gegen den echten Modellkatalog ab.
#
# Entwickler-Werkzeug, kein Laufzeitpfad: es läuft auf dem Rechner des
# Entwicklers, nie in der Sandbox und nie aus dem Daemon heraus. Es überschreibt
# `models.json` nicht. Die Datei beschreibt genau einen Provider,
# `humanitl-local`, und ihr Inhalt ist eine Entscheidung; übernommen wird nur die
# Form. Das Skript sagt deshalb, ob die Form noch stimmt, und überlässt die
# Änderung einem Menschen. Siehe README.md.
set -euo pipefail

BASE_URL="https://models.opencode.ai"
OFFLINE=0
HERE="$(cd "$(dirname "$0")" && pwd)"
SKELETON="$HERE/models.json"

usage() {
    cat <<'USAGE'
usage: update-models.sh [--url BASE] [--offline]

  --url BASE   Basis-Adresse des Katalogs; abgerufen wird BASE/api.json.
               Vorgabe https://models.opencode.ai, Alternative https://models.dev
  --offline    Nur das eigene Skelett prüfen, ohne Netzzugriff.
USAGE
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --url) BASE_URL="${2:?--url braucht eine Adresse}"; shift 2 ;;
        --offline) OFFLINE=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unbekanntes Argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done

for tool in python3; do
    command -v "$tool" >/dev/null 2>&1 || { echo "$tool fehlt" >&2; exit 127; }
done

REMOTE=""
if [[ "$OFFLINE" -eq 0 ]]; then
    command -v curl >/dev/null 2>&1 || { echo "curl fehlt; sonst --offline" >&2; exit 127; }
    REMOTE="$(mktemp)"
    trap 'rm -f "$REMOTE"' EXIT
    echo "lade ${BASE_URL}/api.json"
    curl -fsSL --max-time 30 "${BASE_URL}/api.json" -o "$REMOTE"
fi

SKELETON="$SKELETON" REMOTE="$REMOTE" python3 - <<'PY'
import json
import os
import sys

# Pflichtfelder aus dem Schema der installierten OpenCode-Fassung (1.18.25),
# gelesen aus dem Binary: Provider und Model als Effect-Schema-Struct.
PROVIDER_REQUIRED = {"name", "env", "id", "models"}
PROVIDER_OPTIONAL = {"api", "npm", "doc"}
MODEL_REQUIRED = {
    "id",
    "name",
    "release_date",
    "attachment",
    "reasoning",
    "temperature",
    "tool_call",
    "limit",
}
# Felder, die der Katalog zusätzlich führt und die das Schema von OpenCode
# beim Einlesen fallen lässt. Sie stehen hier, damit das Skript nicht bei jedem
# Lauf über sie stolpert; in `models.json` werden sie nicht gebraucht.
MODEL_OPTIONAL = {
    "family",
    "reasoning_options",
    "interleaved",
    "cost",
    "modalities",
    "experimental",
    "status",
    "provider",
    "description",
    "last_updated",
    "open_weights",
    "structured_output",
    "knowledge",
}
LIMIT_REQUIRED = {"context", "output"}

problems = []


def check(catalog, label):
    """Prüft einen Provider-Eintrag gegen die Pflicht- und Wahlfelder."""
    for provider_id, provider in catalog.items():
        missing = PROVIDER_REQUIRED - set(provider)
        if missing:
            problems.append(f"{label}: Provider {provider_id!r} ohne {sorted(missing)}")
        unknown = set(provider) - PROVIDER_REQUIRED - PROVIDER_OPTIONAL
        if unknown:
            problems.append(f"{label}: Provider {provider_id!r} kennt {sorted(unknown)} nicht")
        for model_id, model in provider.get("models", {}).items():
            missing = MODEL_REQUIRED - set(model)
            if missing:
                problems.append(f"{label}: Modell {model_id!r} ohne {sorted(missing)}")
            unknown = set(model) - MODEL_REQUIRED - MODEL_OPTIONAL
            if unknown:
                problems.append(f"{label}: Modell {model_id!r} kennt {sorted(unknown)} nicht")
            missing = LIMIT_REQUIRED - set(model.get("limit", {}))
            if missing:
                problems.append(f"{label}: Modell {model_id!r} ohne limit.{sorted(missing)}")


with open(os.environ["SKELETON"], encoding="utf-8") as handle:
    skeleton = json.load(handle)
check(skeleton, "models.json")

remote_path = os.environ.get("REMOTE") or ""
if remote_path:
    with open(remote_path, encoding="utf-8") as handle:
        remote = json.load(handle)
    sample_id, sample = next(iter(remote.items()))
    print(f"Vergleichsprovider aus dem Katalog: {sample_id}")
    seen_provider = set(sample)
    grown = seen_provider - PROVIDER_REQUIRED - PROVIDER_OPTIONAL
    if grown:
        problems.append(
            f"Katalog: Provider {sample_id!r} hat neue Felder {sorted(grown)}; "
            "PROVIDER_OPTIONAL in diesem Skript und ggf. models.json nachziehen"
        )
    models = sample.get("models", {})
    if models:
        model_id, model = next(iter(models.items()))
        grown = set(model) - MODEL_REQUIRED - MODEL_OPTIONAL
        if grown:
            problems.append(
                f"Katalog: Modell {model_id!r} hat neue Felder {sorted(grown)}; "
                "MODEL_OPTIONAL in diesem Skript und ggf. models.json nachziehen"
            )
        shrunk = MODEL_REQUIRED - set(model)
        if shrunk:
            print(f"Hinweis: Modell {model_id!r} im Katalog ohne {sorted(shrunk)}")

if problems:
    for line in problems:
        print(f"FAIL {line}", file=sys.stderr)
    sys.exit(1)

print("models.json passt zur Form des Katalogs")
PY
