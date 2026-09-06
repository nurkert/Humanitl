#!/bin/sh
# Der Agent der zweiten Sitzung des M3-Demolaufs, Profil `llm-only`
# (HUM-046, HUM-066, HUM-067).
#
# Diese Sitzung läuft ohne Oberfläche und ohne wartenden Menschen: Die
# Durchreiche zum Sprachmodell trifft in Rang 1, die Regel `block host "**"`
# des Profils deckt alles Übrige, und `ask_mode = "none"` sorgt dafür, dass
# nichts auf eine Entscheidung wartet. Der Agent soll deshalb genau zweierlei
# sehen: seine Inferenz und ein `403`.
#
# Er endet mit 0; was er sah, steht in seinen Zeilen.
set -u

fail() {
    echo "AGENT-ABORT $*"
    exit 97
}

[ -x /usr/bin/curl ] || fail "no curl under /usr/bin in the sandbox"
[ -n "${LLM:-}" ] || fail "sandbox.env did not put LLM into the environment"

# Eine Frist an jeden Aufruf. In dieser Sitzung wartet niemand auf einen
# Menschen (`ask_mode = "none"`, `hold.timeout_secs = 1`), die längste erlaubte
# Antwort ist also der Strom des Mocks mit rund 0,35 s. Zwanzig Sekunden sind
# reichlich; ohne sie bliebe `curl` bei einem Gegenüber hängen, das mitten in
# der Antwort stehenbleibt, und der ganze Lauf liefe in den Abbruch der CI.
DEADLINE=20

echo "LLM1 inference"
curl -sSN --max-time "$DEADLINE" -X POST "$LLM/v1/chat/completions" \
    -H 'content-type: application/json' \
    -d '{"model":"mock","stream":true,"messages":[{"role":"user","content":"llm-only"}]}' \
    > /tmp/llm.sse 2> /tmp/llm.err ||
    echo "LLM1 curl failed: $(cat /tmp/llm.err)"
echo "LLM1 frames=$(grep -c '^data: {' /tmp/llm.sse)"

echo "LLM2 site"
curl -sS --max-time "$DEADLINE" -o /tmp/site.txt \
    -w 'LLM2 status=%{http_code}\n' https://example.com/docs
echo "LLM2 reason=$(grep -i '^reason:' /tmp/site.txt | head -n 1)"

echo "LLM3 done"
