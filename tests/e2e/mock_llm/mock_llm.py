#!/usr/bin/env python3
"""Das Sprachmodell des M3-Demolaufs (HUM-046).

Ein kleiner HTTP-Server, der die Endpunkte bedient, über die ein Coding-Agent
mit einem lokalen Sprachmodell spricht. Er rechnet nichts; er antwortet mit
festen Zeichenketten und tut das, was für den Lauf zählt, wirklich: Er
**streamt**. Die Antwort auf `/v1/chat/completions` mit `stream: true` kommt in
zehn Rahmen im Abstand von `--delay` Millisekunden, jeder mit einem eigenen
`flush`. Eine Durchreiche, die der Proxy in Wahrheit sammelte und erst am Ende
weitergäbe, fiele damit auf: Der Agent misst, wann das erste und wann das
letzte Byte ankam.

Aufruf:

    mock_llm.py --address ADRESSE [--port N] [--chunks N] [--delay MS]

`--port 0` (die Vorgabe) lässt das Betriebssystem einen freien Port wählen;
welcher es wurde, steht in der Bereitschaftszeile. Sobald der Listener steht,
schreibt der Server genau eine Zeile auf stdout:

    READY http=<port>

Danach geht dort nichts mehr hinaus. Das Zugriffsprotokoll läuft nach stderr,
eine Zeile je Anfrage im festen Format

    mock-llm: <methode> <pfad> <status> <rumpfbytes> <stream|plain>

Das Demoskript zählt darin, welche Anfrage den Server wirklich erreicht hat.

# Die Endpunkte

* `GET /api/tags` — die Modellliste im Ollama-Format.
* `GET /v1/models` — dieselbe Auskunft im OpenAI-Format.
* `POST /v1/chat/completions` — mit `"stream": true` ein SSE-Strom aus
  `--chunks` Rahmen `tok0 ` bis `tok<n-1> ` und danach `data: [DONE]`; ohne
  `stream` eine einzelne JSON-Antwort mit dem Inhalt `mock reply`.
* `POST /api/chat` — dasselbe im Ollama-Format, als NDJSON.
* `GET /_debug/last` — der zuletzt empfangene Rumpf, als JSON.

Ein `model`, das dieser Server nicht bedient, bekommt `404` und keine Antwort;
`--model` sagt, welches er bedient (Vorgabe `mock`). Ein Rumpf ohne `model`
gilt als „das eine, das da ist". Ein Testdoppel, das jede Anfrage freundlich
beantwortet, verdeckt genau den Fehler, für den man es hat.

`/_debug/last` ist **nicht** für die Sandbox gedacht, und sie käme dort auch
nicht ohne Weiteres hin: Der Pfad trifft keines der Durchreich-Präfixe aus
`daemon/crates/sandbox/src/agent/opencode.rs`, eine Anfrage von innen würde
also gehalten. Er ist die Gegenprobe des Testprozesses auf dem Wirt: Was der
Agent geschickt zu haben behauptet, steht hier noch einmal, aus der Sicht des
Empfängers.

# Warum Python und eine einzige Datei

Aus demselben Grund wie beim Ziel des M2-Laufs (`backlog/CONVENTIONS.md`
4.22): Eine eigene Crate wäre für einen Server, der feste Zeichenketten
ausgibt, mehr Bauzeit als Nutzen, und `python3` liegt auf Debian wie auf
`ubuntu-latest` bereit. Die erste Fassung der Spezifikation nannte einen
axum-Server; die Abweichung steht in `backlog/CONVENTIONS.md` 4.29.

Dass dieser Server das ganze Milestone trägt, ist der Grund für seine
Sparsamkeit: Er ist in einer Datei zu lesen, er meldet seine Bereitschaft
ausdrücklich, und wenn er nicht steht, bricht der Lauf an der Bereitschaft ab
und nicht später an einer Antwort, die niemand erklären kann.

# Eigener Test ohne Daemon

    tests/e2e/mock_llm/self_test.sh

Startet den Server, zählt die zehn `data:`-Rahmen und prüft die beiden
Auskunftsendpunkte. Ein roter M3-Lauf ist damit von einem kaputten Mock
unterscheidbar.
"""

import argparse
import json
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

# Die Zahl der Rahmen und der Abstand zwischen ihnen, je Server gesetzt.
CHUNKS_ATTR = "humanitl_chunks"
DELAY_ATTR = "humanitl_delay"

# Der zuletzt empfangene Rumpf, für `/_debug/last`.
LAST_ATTR = "humanitl_last"

# Der Name des Modells, das dieser Server bedient.
MODEL_ATTR = "humanitl_model"

# Die Sperre um den gemeinsamen Zustand.
#
# `ThreadingHTTPServer` bedient jede Verbindung in einem eigenen Faden, und
# zwei davon schreiben in dasselbe: den zuletzt empfangenen Rumpf und das
# Zugriffsprotokoll. Ohne Sperre könnten sich zwei Protokollzeilen ineinander
# schieben, und der Lauf zählte in einer Datei, die keine Zeilen mehr hat.
# Heute stellt der Demolauf seine Anfragen nacheinander, aber ein Testdoppel,
# dessen Richtigkeit von der Reihenfolge des Aufrufers abhängt, ist eines, dem
# man beim nächsten Lauf nicht mehr glauben kann.
LOCK_ATTR = "humanitl_lock"

# Der Name des Modells in der Vorgabe. `--model` setzt ihn um.
DEFAULT_MODEL = "mock"


class Mock(BaseHTTPRequestHandler):
    """Bedient die Endpunkte eines lokalen Sprachmodells."""

    # HTTP/1.1, damit der Proxy dieselbe Fassung spricht wie der Klient und
    # eine gestückelte Antwort überhaupt möglich ist.
    protocol_version = "HTTP/1.1"
    server_version = "humanitl-mock-llm/1"
    sys_version = ""

    def do_GET(self):  # noqa: N802 (Name kommt aus BaseHTTPRequestHandler)
        """Beantwortet die Auskunftsendpunkte."""
        path = self.path.split("?", 1)[0]
        model = self._model()
        if path == "/api/tags":
            self._json(
                {
                    "models": [
                        {
                            "name": f"{model}:latest",
                            "modified_at": "2026-09-01T00:00:00Z",
                            "size": 1,
                        }
                    ]
                },
                0,
            )
        elif path == "/v1/models":
            self._json(
                {"object": "list", "data": [{"id": model, "object": "model"}]},
                0,
            )
        elif path == "/_debug/last":
            with self._lock():
                last = getattr(self.server, LAST_ATTR, "")
            self._json({"body": last}, 0)
        else:
            self._json({"error": "not found"}, 0, status=404)

    def do_POST(self):  # noqa: N802
        """Beantwortet die Inferenzendpunkte.

        Ein Modell, das dieser Server nicht bedient, bekommt `404` und keine
        Antwort. Ein Testdoppel, das jede Anfrage freundlich beantwortet,
        verdeckt genau den Fehler, für den man es hat: Wer den Namen des
        Modells vertippt oder ihn aus einer Vorlage nimmt, die etwas anderes
        einsetzt, sähe zehn Rahmen und wüsste nicht, dass sie zu niemandem
        gehören.
        """
        raw = self._read_body()
        with self._lock():
            setattr(self.server, LAST_ATTR, raw.decode("utf-8", "replace"))
        path = self.path.split("?", 1)[0]
        wants_stream = self._wants_stream(raw)
        if path not in ("/v1/chat/completions", "/api/chat"):
            self._json({"error": "not found"}, len(raw), status=404)
            return
        asked = self._asked_model(raw)
        if asked is not None and asked != self._model():
            self._json(
                {"error": f"unknown model: {asked}", "served": self._model()},
                len(raw),
                status=404,
            )
            return
        if path == "/v1/chat/completions":
            if wants_stream:
                self._stream(len(raw), self._openai_frames())
            else:
                self._json(self._openai_reply(), len(raw))
        else:
            if wants_stream:
                self._stream(len(raw), self._ollama_frames())
            else:
                self._json(self._ollama_reply(), len(raw))

    # --- Die Rahmen der beiden Ströme ---------------------------------------

    def _openai_frames(self):
        """Die SSE-Rahmen einer OpenAI-kompatiblen Antwort."""
        for index in range(self._chunks()):
            frame = {"choices": [{"delta": {"content": f"tok{index} "}}]}
            yield f"data: {json.dumps(frame)}\n\n".encode("utf-8")
        yield b"data: [DONE]\n\n"

    def _ollama_frames(self):
        """Die NDJSON-Rahmen einer Ollama-Antwort."""
        for index in range(self._chunks()):
            frame = {"message": {"content": f"tok{index} "}, "done": False}
            yield (json.dumps(frame) + "\n").encode("utf-8")
        yield (json.dumps({"message": {"content": ""}, "done": True}) + "\n").encode(
            "utf-8"
        )

    def _openai_reply(self):
        """Die eine JSON-Antwort ohne Strom."""
        return {
            "id": "mock-1",
            "object": "chat.completion",
            "model": self._model(),
            "choices": [
                {
                    "index": 0,
                    "message": {"role": "assistant", "content": "mock reply"},
                    "finish_reason": "stop",
                }
            ],
        }

    def _ollama_reply(self):
        """Dieselbe Antwort im Ollama-Format."""
        return {
            "model": self._model(),
            "message": {"content": "mock reply"},
            "done": True,
        }

    # --- Das Schreiben ------------------------------------------------------

    def _json(self, value, request_bytes, status=200):
        """Schreibt eine vollständige JSON-Antwort."""
        body = json.dumps(value).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()
        self._access(status, request_bytes, "plain")

    def _stream(self, request_bytes, frames):
        """Schreibt die Rahmen einzeln, mit Pause und `flush` dazwischen.

        Gestückelt (`Transfer-Encoding: chunked`) und von Hand gerahmt: Ein
        `Content-Length` gäbe es erst, wenn alles fertig wäre, und genau das
        soll hier nicht passieren. Jeder Rahmen ist ein eigener HTTP-Chunk,
        also die kleinste Einheit, die der Proxy weiterreichen kann.
        """
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()
        delay = self._delay()
        written = 0
        for index, frame in enumerate(frames):
            if index:
                time.sleep(delay)
            self.wfile.write(f"{len(frame):x}\r\n".encode("ascii"))
            self.wfile.write(frame)
            self.wfile.write(b"\r\n")
            self.wfile.flush()
            written += len(frame)
        self.wfile.write(b"0\r\n\r\n")
        self.wfile.flush()
        self._access(200, request_bytes, "stream")

    # --- Kleinkram ----------------------------------------------------------

    def _read_body(self):
        """Liest den Rumpf vollständig und gibt ihn als Bytes zurück."""
        length = int(self.headers.get("Content-Length") or 0)
        return self.rfile.read(length) if length else b""

    @staticmethod
    def _wants_stream(raw):
        """Ob der Rumpf `"stream": true` sagt."""
        try:
            return bool(json.loads(raw or b"{}").get("stream"))
        except (ValueError, AttributeError):
            return False

    @staticmethod
    def _asked_model(raw):
        """Das Modell aus dem Rumpf, oder `None`, wenn keines darin steht.

        `None` heißt „keine Angabe" und wird bedient: Ein Klient, der kein
        Modell nennt, meint das eine, das der Server hat. Ein Rumpf, der keines
        ist, fällt in denselben Fall — dafür ist der Statuscode nicht da.
        """
        try:
            asked = json.loads(raw or b"{}").get("model")
        except (ValueError, AttributeError):
            return None
        return asked if isinstance(asked, str) and asked else None

    def _model(self):
        """Der Name des Modells, das dieser Server bedient."""
        return getattr(self.server, MODEL_ATTR, DEFAULT_MODEL)

    def _lock(self):
        """Die Sperre um den gemeinsamen Zustand des Servers."""
        return getattr(self.server, LOCK_ATTR)

    def _chunks(self):
        """Wie viele Rahmen ein Strom hat."""
        return getattr(self.server, CHUNKS_ATTR, 10)

    def _delay(self):
        """Der Abstand zwischen zwei Rahmen, in Sekunden."""
        return getattr(self.server, DELAY_ATTR, 0.03)

    def _access(self, status, request_bytes, kind):
        """Eine Zeile des Zugriffsprotokolls, im festen Format.

        Unter der Sperre: Zwei Fäden, die gleichzeitig schreiben, könnten sonst
        eine Zeile in die andere schieben, und der Demolauf zählt in dieser
        Datei.
        """
        line = "mock-llm: {} {} {} {} {}\n".format(
            self.command, self.path, status, request_bytes, kind
        )
        with self._lock():
            sys.stderr.write(line)
            sys.stderr.flush()

    def log_message(self, fmt, *args):
        """Unterdrückt das eingebaute Protokoll; `_access` schreibt es selbst."""

    def log_error(self, fmt, *args):
        """Fehler des Servers gehören nach stderr, aber ohne den Zeitstempel."""
        sys.stderr.write("mock-llm: error " + (fmt % args) + "\n")
        sys.stderr.flush()


def main(argv):
    """Startet den Listener und meldet, sobald er steht."""
    parser = argparse.ArgumentParser(description="the language model of the M3 demo run")
    parser.add_argument("--address", required=True)
    parser.add_argument("--port", type=int, default=0)
    parser.add_argument("--chunks", type=int, default=10)
    parser.add_argument("--delay", type=int, default=30, help="milliseconds")
    parser.add_argument("--model", default=DEFAULT_MODEL)
    args = parser.parse_args(argv[1:])

    server = ThreadingHTTPServer((args.address, args.port), Mock)
    setattr(server, CHUNKS_ATTR, args.chunks)
    setattr(server, DELAY_ATTR, args.delay / 1000.0)
    setattr(server, LAST_ATTR, "")
    setattr(server, MODEL_ATTR, args.model)
    setattr(server, LOCK_ATTR, threading.Lock())

    sys.stdout.write(f"READY http={server.server_address[1]}\n")
    sys.stdout.flush()
    server.serve_forever()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
