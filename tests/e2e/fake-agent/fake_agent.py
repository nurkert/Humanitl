#!/usr/bin/env python3
"""Der Agent des M2-Demolaufs (HUM-036).

Er tut, was ein echter Agent in der Sandbox tut, und sonst nichts: Er stellt
HTTP-Anfragen über die eine Tür, die es dort gibt, und schreibt auf, was
zurückkam.

    fake_agent.py SCRIPT.json

`SCRIPT.json` beschreibt die Anfragen mit ihren Zeitpunkten, relativ zum Start
des Agenten und in Millisekunden:

    {"steps": [
      {"at": 0,    "req": {"method": "GET",  "url": "http://registry.npmjs.org/left-pad"}},
      {"at": 4200, "req": {"method": "POST", "url": "http://api.github.com/graphql",
                           "body": "{\\"q\\": \\"…\\"}"}}
    ]}

Je Schritt ein Thread: Er wartet bis zu seinem Zeitpunkt und stellt dann die
Anfrage, gleich ob eine frühere noch hängt. Genau darauf kommt es an — die
zwölf Anfragen an die Paket-Registry sollen sich in der Warteschlange
sammeln, während die erste noch auf einen Menschen wartet. Ein Skript, das
seine Anfragen nacheinander stellte, hätte nie mehr als eine gehaltene Anfrage
und könnte die Gruppierung nicht zeigen.

Auf stdout steht je Anfrage eine Zeile JSON, in der Reihenfolge, in der die
Antworten eintrafen:

    {"index": 0, "at": 0, "method": "GET", "url": "…", "status": 200,
     "ms": 6123, "curl_exit": 0, "body_head": "…"}

`status` ist der HTTP-Status, den der Agent gesehen hat (0, wenn keiner kam),
`ms` die Dauer der Anfrage aus Sicht des Agenten, `body_head` der Anfang der
Antwort. Der Anfang genügt: Die Blockantwort des Proxys nennt ihren Grund in
den ersten Zeilen, und mehr braucht das Demoskript nicht.

Gesprochen wird über `curl`, nicht über eine eigene HTTP-Bibliothek. Der Grund
ist derselbe wie beim Demoskript, das über `humanitl` fährt und nicht über
einen eigenen gRPC-Client: Gemessen werden soll der Weg, den ein echter Agent
nimmt. `curl` liest `HTTP_PROXY`, `HTTPS_PROXY` und `CURL_CA_BUNDLE` aus dem
Umgebungs-Kit des Sandbox-Profils, löst deshalb selbst keinen Namen auf und
spricht ausschließlich mit dem Proxy auf `127.0.0.1:3128`.

Vorausgesetzt werden in der Sandbox nur `python3` und `curl`; beide liegen
unter `/usr`, das jedes Profil nur lesbar einhängt, und der M1-Lauf fährt seine
Anfragen bereits mit demselben `curl`. Fehlt eines von beiden, endet der Agent
sofort mit einer Meldung statt mit einer Reihe stiller Fehlschläge.
"""

import json
import os
import shutil
import subprocess
import sys
import threading
import time

# Der Klient, mit dem der Agent spricht. Ein fester Pfad, kein PATH-Fund: In
# der Sandbox steht genau dieses Binary, und ein anderes wäre eine Überraschung.
CURL = "/usr/bin/curl"

# Obergrenze je Anfrage. Sie liegt weit über der Haltefrist des Demolaufs, denn
# eine Anfrage, die auf einen Menschen wartet, ist nicht hängengeblieben.
REQUEST_TIMEOUT_SECS = 90

# So viele Zeichen der Antwort stehen in der Ausgabezeile.
BODY_HEAD_CHARS = 400

# Schützt stdout: Die Threads schreiben ihre Zeilen unabhängig voneinander.
OUT_LOCK = threading.Lock()


def emit(record):
    """Schreibt eine Ergebniszeile, vollständig und ohne Verschränkung."""
    line = json.dumps(record, sort_keys=True)
    with OUT_LOCK:
        sys.stdout.write(line + "\n")
        sys.stdout.flush()


def note(text):
    """Schreibt eine Zeile in das Protokoll des Agenten (stderr)."""
    sys.stderr.write("fake-agent: " + text + "\n")
    sys.stderr.flush()


def run_request(index, step, out_dir):
    """Stellt eine Anfrage und gibt die Ergebniszeile zurück."""
    req = step.get("req", {})
    method = req.get("method", "GET")
    url = req.get("url", "")
    body = req.get("body")
    out_file = os.path.join(out_dir, f"response-{index:03d}.txt")

    argv = [
        CURL,
        "-sS",
        "--max-time",
        str(REQUEST_TIMEOUT_SECS),
        "-o",
        out_file,
        "-w",
        "%{http_code}",
        "-X",
        method,
        url,
    ]
    if body is not None:
        argv[-1:-1] = ["-H", "Content-Type: application/json", "--data-binary", "@-"]

    started = time.monotonic()
    completed = subprocess.run(
        argv,
        input=(body.encode("utf-8") if body is not None else b""),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    elapsed_ms = int((time.monotonic() - started) * 1000)

    status = 0
    written = completed.stdout.decode("utf-8", "replace").strip()
    if written.isdigit():
        status = int(written)

    body_head = ""
    try:
        with open(out_file, "r", encoding="utf-8", errors="replace") as handle:
            body_head = handle.read(BODY_HEAD_CHARS)
    except OSError:
        body_head = ""

    return {
        "index": index,
        "at": step.get("at", 0),
        "method": method,
        "url": url,
        "status": status,
        "ms": elapsed_ms,
        "curl_exit": completed.returncode,
        "stderr": completed.stderr.decode("utf-8", "replace").strip()[:200],
        "body_head": body_head,
    }


def schedule(index, step, start, out_dir):
    """Wartet bis zum Zeitpunkt des Schritts und stellt dann die Anfrage."""
    delay = (step.get("at", 0) / 1000.0) - (time.monotonic() - start)
    if delay > 0:
        time.sleep(delay)
    emit(run_request(index, step, out_dir))


def main(argv):
    """Liest das Skript, fährt es ab und wartet auf die letzte Antwort."""
    if len(argv) != 2:
        note("usage: fake_agent.py SCRIPT.json")
        return 2
    if not os.path.isfile(CURL) or not os.access(CURL, os.X_OK):
        note(f"{CURL} is missing in the sandbox; the agent cannot send anything")
        return 3
    if shutil.which("sh") is None:
        note("the sandbox has no /bin/sh; this is not the profile the demo expects")

    try:
        with open(argv[1], "r", encoding="utf-8") as handle:
            script = json.load(handle)
    except (OSError, ValueError) as error:
        note(f"cannot read {argv[1]}: {error}")
        return 2

    steps = script.get("steps", [])
    if not steps:
        note(f"{argv[1]} has no steps")
        return 2

    out_dir = os.environ.get("HUMANITL_AGENT_OUT", "/tmp/fake-agent")
    try:
        os.makedirs(out_dir, exist_ok=True)
    except OSError as error:
        note(f"cannot create {out_dir}: {error}")
        return 2

    note(f"{len(steps)} steps, responses in {out_dir}")
    start = time.monotonic()
    threads = []
    for index, step in enumerate(steps):
        thread = threading.Thread(
            target=schedule, args=(index, step, start, out_dir), daemon=False
        )
        thread.start()
        threads.append(thread)
    for thread in threads:
        thread.join()
    note("every step is done")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
