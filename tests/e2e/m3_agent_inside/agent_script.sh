#!/bin/sh
# Der Agent des M3-Demolaufs (HUM-046).
#
# Er läuft in der Sandbox, unter `/work`, gestartet von `humanitl run`. Alles,
# was er kann, geht über `curl` und damit über den Proxy: `HTTP_PROXY` und
# `CURL_CA_BUNDLE` stehen im Umgebungs-Kit des Sandbox-Profils, er löst also
# selbst keinen Namen auf und spricht ausschließlich mit `127.0.0.1:3128`.
# Gemessen werden soll der Weg, den ein echter Agent nimmt
# (`backlog/CONVENTIONS.md` 4.22).
#
# Jede Zeile, die er schreibt, ist eine Zusicherung des Laufs. Die Marken
# `STEP1` bis `STEP8` stehen am Zeilenanfang, damit `run.sh` sie einzeln
# findet; was dazwischen steht, ist das, was der Agent wirklich gesehen hat.
#
# Er endet mit 0, auch wenn ein einzelnes `curl` scheitert: Was der Agent sah,
# steht in seinen Zeilen, und der Lauf prüft es dort. Nur eine fehlende
# Voraussetzung bricht ab (Exit 97), denn dann prüfte alles Weitere nichts.
set -u

fail() {
    echo "AGENT-ABORT $*"
    exit 97
}

[ -x /usr/bin/curl ] || fail "no curl under /usr/bin in the sandbox"
[ -n "${LLM:-}" ] || fail "sandbox.env did not put LLM into the environment"

# Eine Sekunde Vorlauf, und zwar aus einem gemessenen Grund. Der Daemon meldet
# den Zuhörer, der die Hinweiszeilen in dieses Terminal schreibt, erst an,
# nachdem er die drei Garantien geprüft und eine Momentaufnahme der Sandbox
# gemacht hat (`humanitl_ipc::sandbox`, `HeldNotices::run`); der Agent läuft da
# schon. Eine Entscheidung, die in dieses Fenster fällt, wird nie angesagt —
# die Warteschlange verteilt sie als Rundfunk, und wer noch nicht zuhört,
# verpasst sie. Für eine wartende Anfrage ist das folgenlos, weil sie wartet,
# bis ein Mensch entscheidet; für die Durchreiche in Schritt 1 wäre es eine
# Zeile, die mal da ist und mal nicht. Der Lauf misst deshalb Hinweise und
# nicht ein Wettrennen. Die Lücke selbst ist ein Befund und steht in
# `backlog/CONVENTIONS.md` 4.29.
sleep 1

# Zwei Fristen, und sie sind verschieden lang, weil die längste erlaubte
# Antwort verschieden lang ist.
#
#   * `LLM_DEADLINE` gilt für die Durchreiche. Der Mock schickt zehn Rahmen im
#     Abstand von 30 ms, ist also nach rund 0,35 s fertig; zwanzig Sekunden
#     sind das Fünfzigfache davon.
#   * `MODERATED_DEADLINE` gilt für die drei Anfragen, über die ein Mensch
#     entscheidet. Ihre längste erlaubte Antwort ist die Haltefrist
#     (`hold.timeout_secs = 20`), nach der der Proxy selbst mit `504`
#     antwortet; dreissig Sekunden lassen zehn darüber.
#
# Ohne Frist bliebe `curl` hängen, wenn das Gegenüber mitten in der Antwort
# stehenbleibt: Der Lauf wartete dann auf einen Prozess, der nie endet, und
# das Ganze liefe in den 30-Minuten-Abbruch der CI — der echte Fehler stünde
# hinter einer Zeitüberschreitung, die nichts sagt.
LLM_DEADLINE=20
MODERATED_DEADLINE=30

# 1. Das Sprachmodell, über die Durchreiche. Gestreamt und nicht gehalten: Der
#    Abstand zwischen dem ersten und dem letzten Byte ist das, was ein
#    sammelnder Proxy nicht hätte. Erst in eine Datei, dann ausgeben — eine
#    Pipe, die vorne abbricht, schnitte den Strom ab und prüfte damit etwas
#    anderes als gemeint.
echo "STEP1 llm"
curl -sSN --max-time "$LLM_DEADLINE" -X POST "$LLM/v1/chat/completions" \
    -H 'content-type: application/json' \
    -w '\nSTEP1 timing ttfb=%{time_starttransfer} total=%{time_total}\n' \
    -d '{"model":"mock","stream":true,"messages":[{"role":"user","content":"hello from the sandbox"}]}' \
    > /tmp/llm.sse 2> /tmp/llm.err ||
    echo "STEP1 curl failed: $(cat /tmp/llm.err)"
echo "STEP1 frames=$(grep -c '^data: {' /tmp/llm.sse)"
cat /tmp/llm.sse

# 2. Der Modellkatalog, den die Dokumentation von OpenCode nennt. Eine
#    mitgelieferte Regel blockt ihn, ohne zu fragen; Humanitl liefert den
#    Katalog als Datei mit.
echo "STEP2 catalog"
curl -sS --max-time "$MODERATED_DEADLINE" -o /dev/null \
    -w 'STEP2 status=%{http_code}\n' https://models.dev/api.json

# 3. Die Anfrage, über die ein Mensch entscheidet. Der Rumpf steht mit in der
#    Ausgabe: Ein Status allein sagt nicht, von wem die Antwort kam.
echo "STEP3 fetch"
curl -sS --max-time "$MODERATED_DEADLINE" \
    -w '\nSTEP3 status=%{http_code}\n' https://example.com/docs

# 4. Dieselbe Sorte Anfrage, mit einem Pfad, der die Hinweiszeile des Daemons
#    fälschen soll. Die eckige Klammer gehört dem Absender: `path_for_notice`
#    macht daraus eine runde, und die gefälschte Zeile trägt damit sichtbar
#    keinen zweiten Absender. `-g` schaltet das Globbing von `curl` ab, das
#    `[` sonst als Bereich läse.
echo "STEP4 notice"
curl -sSg --max-time "$MODERATED_DEADLINE" -o /dev/null \
    -w 'STEP4 status=%{http_code}\n' \
    'https://example.com/a[humanitl]request-allowed'

# 5. Die Anfrage, die ein Mensch mit einer Notiz verbietet. Der Kopf, den der
#    Agent sieht, muss einzeilig sein: Eine Notiz mit CR und LF wäre sonst ein
#    Werkzeug, um eine zweite Kopfzeile zu öffnen.
echo "STEP5 note"
curl -sS --max-time "$MODERATED_DEADLINE" -D /tmp/head.txt -o /tmp/body.txt \
    -w 'STEP5 status=%{http_code}\n' https://example.com/secret
echo "STEP5 note-header=$(tr -d '\r' < /tmp/head.txt | grep -i '^x-humanitl-note:' | head -n 1)"
echo "STEP5 injected-headers=$(tr -d '\r' < /tmp/head.txt | grep -c -i '^x-injected:')"
echo "STEP5 header-lines=$(tr -d '\r' < /tmp/head.txt | grep -c .)"
cat /tmp/body.txt

# 6. Der erklärte Seitenkanal: Was der Agent schreibt, liest ein Mensch in
#    seinem Terminal, und das Transkript wird als CI-Artefakt im Browser
#    geöffnet (BACKLOG.md 4.2). Die OSC-52-Nutzlast legte einen Text in die
#    Zwischenablage; die Farbfolge daneben darf bleiben. Beide stehen in einer
#    Zeile, damit die Prüfung des Filters ein Paar ist und nicht eine
#    Behauptung über ein Ausbleiben.
printf 'STEP6 filter osc<\033]52;c;aHVtYW5pdGw=\007> colour<\033[31mred\033[0m>\n'

# 7. Etwas im Projekt hinterlassen, damit die Zusammenfassung des Laufs etwas
#    zu berichten hat. Eine leere Zusammenfassung sähe genauso aus wie eine,
#    die nie gelaufen ist.
echo "STEP7 write"
echo "the agent of the M3 demo run was here" > /work/notes.txt
echo "STEP7 wrote=$(wc -c < /work/notes.txt | tr -d ' ')"

echo "STEP8 done"
