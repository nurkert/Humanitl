#!/usr/bin/env sh
# Der eigene Test des Mock-Sprachmodells (HUM-046).
#
#   tests/e2e/mock_llm/self_test.sh
#
# Er braucht weder Daemon noch Sandbox noch Netz-Namensraum: Er startet
# `mock_llm.py` auf dem Loopback und spricht mit `curl` direkt mit ihm. Sein
# Zweck ist die Unterscheidung, ohne die ein roter M3-Lauf nichts sagt — liegt
# es am Produkt oder am Testdouble? Der Mock trägt das ganze Milestone; wenn er
# kaputt ist, soll das hier auffallen und nicht dort.
#
# Geprüft wird, was der M3-Lauf von ihm annimmt:
#
#   1. Er meldet seine Bereitschaft mit dem Port, den er wirklich bekommen hat.
#   2. `GET /api/tags` und `GET /v1/models` geben die beiden Modellauskünfte.
#   3. `POST /v1/chat/completions` mit `stream: true` liefert genau zehn
#      `data:`-Rahmen plus `data: [DONE]`, und die Rahmen kommen verteilt über
#      die Zeit und nicht auf einmal.
#   4. Dieselbe Anfrage ohne `stream` liefert eine einzelne Antwort.
#   5. `GET /_debug/last` gibt den zuletzt empfangenen Rumpf zurück.
#
# Exit-Codes: 0 alles gehalten, 1 eine Behauptung hielt nicht.
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
ADDR="127.0.0.1"
CHECKS=0
FAILED=0

# Eine Frist an jedem Aufruf. Der Server steht auf dem Loopback und ist nach
# rund 0,35 s fertig; zehn Sekunden sind das Dreissigfache. Ohne Frist bliebe
# `curl` bei einem Server hängen, der mitten in der Antwort stehenbleibt — und
# genau das ist einer der Fehler, für die es diesen Test gibt.
DEADLINE=10

say() {
    printf 'mock-llm self test: %s\n' "$*"
}

check() {
    CHECKS=$((CHECKS + 1))
    if [ "$2" = "$3" ]; then
        printf '  ok    %s\n' "$1"
    else
        printf '  FAIL  %s: expected %s, got %s\n' "$1" "$2" "$3" >&2
        FAILED=$((FAILED + 1))
    fi
}

for tool in curl python3; do
    command -v "$tool" > /dev/null 2>&1 || {
        printf 'mock-llm self test: %s is missing\n' "$tool" >&2
        exit 1
    }
done

WORK=$(mktemp -d /tmp/hum-mockllm-XXXXXX)
PID=""

cleanup() {
    if [ -n "$PID" ]; then
        kill "$PID" 2> /dev/null || true
        wait "$PID" 2> /dev/null || true
    fi
    rm -rf "$WORK"
}
trap cleanup EXIT
trap 'printf "mock-llm self test: interrupted\n" >&2; trap - EXIT; cleanup; exit 130' INT TERM HUP

# Die Bereitschaft kommt über eine Datei zurück, mit Frist und mit einem Blick
# auf den Prozess. Eine Fifo ohne Frist wartete unbegrenzt — schon beim Öffnen
# —, und ein Mock, der gar nicht hochkommt, ließe diesen Test hängen, statt zu
# sagen, dass er nicht hochkam. Genau dafür gibt es ihn.
: > "$WORK/ready"
python3 "$HERE/mock_llm.py" --address "$ADDR" --port 0 --chunks 10 --delay 30 \
    > "$WORK/ready" 2> "$WORK/access.log" &
PID=$!
READY=""
LEFT=200
while [ "$LEFT" -gt 0 ]; do
    READY=$(grep -m 1 '^READY ' "$WORK/ready" 2> /dev/null || true)
    [ -z "$READY" ] || break
    if ! kill -0 "$PID" 2> /dev/null; then
        printf 'mock-llm self test: the mock died before it was ready: %s\n' \
            "$(cat "$WORK/access.log" 2> /dev/null)" >&2
        exit 1
    fi
    sleep 0.1
    LEFT=$((LEFT - 1))
done
PORT="${READY#READY http=}"
if [ "$READY" = "$PORT" ] || [ -z "$PORT" ]; then
    printf 'mock-llm self test: no ready line within twenty seconds (got "%s"): %s\n' \
        "$READY" "$(cat "$WORK/access.log" 2> /dev/null)" >&2
    exit 1
fi
BASE="http://$ADDR:$PORT"
say "up on $BASE"

# 1. Die beiden Auskunftsendpunkte, am Inhalt gemessen und nicht am Status: Ein
#    Server, der auf alles `200` sagt, bestünde eine Statusprüfung.
check "the ollama tag list names the model" "mock:latest" \
    "$(curl -sS --max-time "$DEADLINE" "$BASE/api/tags" | python3 -c 'import json,sys; print(json.load(sys.stdin)["models"][0]["name"])')"
check "the openai model list names the model" "mock" \
    "$(curl -sS --max-time "$DEADLINE" "$BASE/v1/models" | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"][0]["id"])')"

# 2. Der Strom. Gezählt werden die Rahmen, nicht die Bytes.
curl -sSN --max-time "$DEADLINE" -X POST "$BASE/v1/chat/completions" \
    -H 'content-type: application/json' \
    -w '\nTIMING %{time_starttransfer} %{time_total}\n' \
    -d '{"model":"mock","stream":true,"messages":[{"role":"user","content":"hello"}]}' \
    > "$WORK/stream.txt"

check "the stream carries ten token frames" 10 \
    "$(grep -c '^data: {' "$WORK/stream.txt" || true)"
check "and the first token is tok0" 1 \
    "$(grep -c 'tok0 ' "$WORK/stream.txt" || true)"
check "and the last one is tok9" 1 \
    "$(grep -c 'tok9 ' "$WORK/stream.txt" || true)"
check "and it ends with the done frame" 1 \
    "$(grep -c '^data: \[DONE\]$' "$WORK/stream.txt" || true)"

# Und die Rahmen kamen verteilt: zehn Pausen à 30 ms sind mindestens 0,27 s
# zwischen dem ersten und dem letzten Byte. Ein Server, der alles auf einmal
# ausgäbe, hielte die zehn Rahmen ein und diese Behauptung nicht.
check "and they arrive spread over time, not in one piece" yes \
    "$(awk '/^TIMING/ { print ($3 - $2 >= 0.15) ? "yes" : "no (" $3 - $2 "s)" }' "$WORK/stream.txt")"

# 3. Dieselbe Anfrage ohne Strom.
check "without stream the answer is one json object" "mock reply" \
    "$(curl -sS --max-time "$DEADLINE" -X POST "$BASE/v1/chat/completions" \
        -H 'content-type: application/json' \
        -d '{"model":"mock","messages":[{"role":"user","content":"hi"}]}' |
        python3 -c 'import json,sys; print(json.load(sys.stdin)["choices"][0]["message"]["content"])')"

# 4. Der Ollama-Strom, den ein Agent mit dem nativen Protokoll nimmt.
check "the ollama stream carries ten frames" 10 \
    "$(curl -sSN --max-time "$DEADLINE" -X POST "$BASE/api/chat" \
        -H 'content-type: application/json' \
        -d '{"model":"mock","stream":true,"messages":[{"role":"user","content":"hello"}]}' |
        grep -c '"done": false' || true)"

# 5. Die Gegenprobe des Wirts: Was zuletzt ankam, steht unter `/_debug/last`.
check "the debug endpoint holds the last body" 1 \
    "$(curl -sS --max-time "$DEADLINE" "$BASE/_debug/last" | grep -c 'hello' || true)"

# 6. Ein Modell, das dieser Server nicht bedient, wird abgelehnt. Das Paar
#    dazu steht oben: Dieselbe Anfrage mit dem richtigen Namen bekommt ihre
#    zehn Rahmen. Ein Testdoppel, das jede Anfrage freundlich beantwortet,
#    verdeckt genau den Fehler, für den man es hat — einen Modellnamen, den
#    eine Vorlage anders einsetzt als gedacht.
check "an unknown model is refused" 404 \
    "$(curl -sS --max-time "$DEADLINE" -o /dev/null -w '%{http_code}' \
        -X POST "$BASE/v1/chat/completions" -H 'content-type: application/json' \
        -d '{"model":"not-the-one","messages":[{"role":"user","content":"hi"}]}')"
check "and the refusal says which model it does serve" "mock" \
    "$(curl -sS --max-time "$DEADLINE" -X POST "$BASE/v1/chat/completions" \
        -H 'content-type: application/json' \
        -d '{"model":"not-the-one","messages":[{"role":"user","content":"hi"}]}' |
        python3 -c 'import json,sys; print(json.load(sys.stdin)["served"])')"

# Und der Server hat mitgeschrieben, was er bedient hat.
check "the access log has one line per request" 8 \
    "$(grep -c '^mock-llm: ' "$WORK/access.log" || true)"

if [ "$FAILED" -gt 0 ]; then
    say "$FAILED of $CHECKS checks failed"
    exit 1
fi
say "OK, $CHECKS checks"
