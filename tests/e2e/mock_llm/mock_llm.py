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
* `GET /_debug/tool` — was ein Werkzeug dem Agenten zuletzt geliefert hat, als
  JSON (`{"content": …}`).

# Der Werkzeugaufruf, auf Verlangen (HUM-141)

Mit zehn Token ruft kein Agent je ein Werkzeug auf. Steht in der letzten
Nachricht des Nutzers das Wort `humanitl-webfetch` und dahinter eine URL, und
bietet die Anfrage ein Werkzeug an, dessen Name `webfetch` enthält, antwortet
`/v1/chat/completions` stattdessen mit einem Aufruf dieses Werkzeugs, gestreamt
oder als eine Antwort, im Format der OpenAI-API (`tool_calls`,
`finish_reason: "tool_calls"`). Kommt danach die Antwort des Werkzeugs zurück,
also eine letzte Nachricht mit `role: "tool"`, antwortet der Server mit einem
Satz, der sie wiedergibt, und hält ihren Inhalt unter `/_debug/tool` bereit.
Ohne das Wort oder ohne das Werkzeug bleibt alles, wie es war.

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
import re
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

# Das Wort, das einen Werkzeugaufruf verlangt, und das Werkzeug, das gemeint
# ist. Ein Wort, das in keiner gewöhnlichen Frage steht, damit kein anderer
# Schritt des Laufs versehentlich einen Aufruf bekommt.
TOOL_MARKER = "humanitl-webfetch"
TOOL_NEEDLE = "webfetch"

# Was ein Werkzeug dem Agenten zuletzt geliefert hat, für `/_debug/tool`.
TOOL_ATTR = "humanitl_tool_result"
# Die Kennung des einen Aufrufs, den der Mock ausgibt. Nur eine Werkzeugantwort
# mit genau dieser Kennung ist die Antwort auf ihn.
TOOL_CALL_ID = "call_humanitl_1"


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
        elif path == "/_debug/tool":
            with self._lock():
                result = getattr(self.server, TOOL_ATTR, "")
            self._json({"content": result}, 0)
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
        if path == "/v1/chat/completions":
            # Eine eigene Zeile mit eigenem Präfix: Die Zeilen `mock-llm:`
            # zählt und vergleicht der Lauf als Ganzes, und ein Anhang an sie
            # brach dort ein Muster (gemessen im M3-Lauf, Schritt 4).
            with self._lock():
                sys.stderr.write("mock-llm-note:" + _request_note(raw) + "\n")
                sys.stderr.flush()
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
            plan = self._tool_plan(raw)
            if plan is not None and wants_stream:
                self._stream(len(raw), _planned_frames(plan))
            elif plan is not None:
                self._json(self._planned_reply(plan), len(raw))
            elif wants_stream:
                self._stream(len(raw), self._openai_frames())
            else:
                self._json(self._openai_reply(), len(raw))
        else:
            if wants_stream:
                self._stream(len(raw), self._ollama_frames())
            else:
                self._json(self._ollama_reply(), len(raw))

    # --- Der Werkzeugaufruf (HUM-141) ---------------------------------------

    def _tool_plan(self, raw):
        """Was diese Anfrage statt der zehn Token bekommt, oder `None`.

        `("call", name, url)`, wenn die letzte Nachricht des Nutzers
        `TOOL_MARKER` und eine URL trägt und ein passendes Werkzeug angeboten
        ist; `("answer", text)`, wenn die letzte Nachricht die Antwort eines
        Werkzeugs auf den eigenen Aufruf ist (`TOOL_CALL_ID`). Deren Inhalt
        steht danach unter `/_debug/tool`: Er ist der Beleg dafür, was beim
        Agenten ankam, und nicht, was der Agent darüber schreibt.
        """
        try:
            request = json.loads(raw or b"{}")
        except ValueError:
            return None
        messages = request.get("messages") if isinstance(request, dict) else None
        if not isinstance(messages, list) or not messages or not isinstance(messages[-1], dict):
            return None
        last = messages[-1]
        if last.get("role") == "tool":
            # Nur die Antwort auf den eigenen Aufruf. Ein Werkzeug, das der
            # Agent von sich aus gerufen hat, bekommt die zehn Token wie jede
            # andere Frage; sonst änderte der Mock den gewöhnlichen Weg für
            # jeden Lauf, in dem ein Agent ein Werkzeug benutzt.
            if last.get("tool_call_id") != TOOL_CALL_ID:
                return None
            content = _text(last.get("content"))
            with self._lock():
                setattr(self.server, TOOL_ATTR, content)
            return ("answer", "tool said: " + content[:200])
        # Die letzte Nachricht des Nutzers, nicht die letzte überhaupt: Ein
        # Agent schickt nach ihr eigene Nachrichten mit. Kam nach ihr schon
        # die Antwort eines Werkzeugs, ist der Aufruf erledigt.
        user = _last_user(messages)
        if user is None:
            return None
        position, text = user
        if any(
            isinstance(later, dict) and later.get("role") == "tool"
            for later in messages[position + 1 :]
        ):
            return None
        url = _marked_url(text)
        name = _offered_tool(request.get("tools"), TOOL_NEEDLE)
        if url is None or name is None:
            return None
        return ("call", name, url)

    def _planned_reply(self, plan):
        """Die eine JSON-Antwort zu einem Plan aus [_tool_plan]."""
        if plan[0] == "call":
            message = {"role": "assistant", "content": None, "tool_calls": [_call(plan[1], plan[2])]}
            finish = "tool_calls"
        else:
            message = {"role": "assistant", "content": plan[1]}
            finish = "stop"
        return {
            "id": "mock-tool-1",
            "object": "chat.completion",
            "model": self._model(),
            "choices": [{"index": 0, "message": message, "finish_reason": finish}],
        }

    # --- Die Rahmen der beiden Ströme ---------------------------------------

    def _openai_frames(self):
        """Die SSE-Rahmen einer OpenAI-kompatiblen Antwort.

        Der letzte Rahmen trägt `finish_reason: "stop"`, wie jeder Strom der
        OpenAI-API. Ohne ihn hält ein Agent die Antwort für unfertig und fragt
        mit ihr als letzter Nachricht erneut; gemessen am 2026-09-11 mit
        OpenCode, 289 von 295 Anfragen (HUM-141). Auf dem letzten Rahmen und
        nicht in einem eigenen: Drei Stellen im Lauf zählen genau zehn
        `data: {`-Rahmen.
        """
        chunks = self._chunks()
        for index in range(chunks):
            choice = {"delta": {"content": f"tok{index} "}}
            if index == chunks - 1:
                choice["finish_reason"] = "stop"
            yield f"data: {json.dumps({'choices': [choice]})}\n\n".encode("utf-8")
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


def _request_note(raw):
    """Was an einer Chat-Anfrage für den Werkzeugaufruf zählt, fürs Protokoll.

    Steht als eigene Zeile `mock-llm-note:` im Protokoll: welche Werkzeuge
    angeboten wurden, welche Rolle die letzte Nachricht hat und ob das Wort
    `TOOL_MARKER` darin steht. Ohne das bleibt ein Lauf, in dem der Aufruf nie
    kam, eine Vermutung (HUM-141). Die Zeilen `mock-llm:` bleiben, wie sie
    waren.
    """
    try:
        request = json.loads(raw or b"{}")
    except ValueError:
        return " tools=? last=?"
    if not isinstance(request, dict):
        return " tools=? last=?"
    names = []
    for tool in request.get("tools") or []:
        function = tool.get("function") if isinstance(tool, dict) else None
        name = function.get("name") if isinstance(function, dict) else None
        if isinstance(name, str):
            names.append(name)
    messages = request.get("messages") if isinstance(request.get("messages"), list) else []
    last = messages[-1] if messages else {}
    role = last.get("role", "?") if isinstance(last, dict) else "?"
    user = _last_user(messages)
    text = user[1] if user else ""
    marked = _marked_url(text) is not None
    return " tools={} last={} marker={} user={}".format(
        ",".join(names) or "-", role, "yes" if marked else "no", json.dumps(text[:160])
    )


def _last_user(messages):
    """Stelle und Text der letzten Nachricht des Nutzers, oder `None`."""
    for position in range(len(messages) - 1, -1, -1):
        message = messages[position]
        if isinstance(message, dict) and message.get("role") == "user":
            return position, _text(message.get("content"))
    return None


def _marked_url(text):
    """Die URL hinter `TOOL_MARKER`, auch mitten im Text, oder `None`.

    Ein Agent verpackt die Frage des Nutzers: in Anführungszeichen, in eine
    Vorlage, mit Satzzeichen daneben. Das Wort muss deshalb nicht allein
    stehen; die URL ist die erste nach ihm.
    """
    start = text.find(TOOL_MARKER)
    if start < 0:
        return None
    found = re.search(r"https?://[^\s\"'<>]+", text[start + len(TOOL_MARKER) :])
    return found.group(0) if found else None


def _text(content):
    """Der Text einer Nachricht: ein String oder eine Liste von Teilen."""
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return " ".join(
            part.get("text", "") for part in content if isinstance(part, dict)
        )
    return ""


def _offered_tool(tools, needle):
    """Der Name des ersten angebotenen Werkzeugs, der [needle] enthält."""
    for tool in tools if isinstance(tools, list) else []:
        function = tool.get("function") if isinstance(tool, dict) else None
        name = function.get("name") if isinstance(function, dict) else None
        if isinstance(name, str) and needle in name.lower():
            return name
    return None


def _call(name, url):
    """Ein Werkzeugaufruf im Format der OpenAI-API."""
    return {
        "index": 0,
        "id": TOOL_CALL_ID,
        "type": "function",
        "function": {"name": name, "arguments": json.dumps({"url": url, "format": "text"})},
    }


def _sse(value):
    """Ein SSE-Rahmen aus einem JSON-Wert."""
    return f"data: {json.dumps(value)}\n\n".encode("utf-8")


def _planned_frames(plan):
    """Die Rahmen eines gestreamten Plans: der Aufruf oder der Satz, dann das Ende."""
    if plan[0] == "call":
        delta = {"role": "assistant", "tool_calls": [_call(plan[1], plan[2])]}
        finish = "tool_calls"
    else:
        delta = {"role": "assistant", "content": plan[1]}
        finish = "stop"
    yield _sse({"choices": [{"index": 0, "delta": delta}]})
    yield _sse({"choices": [{"index": 0, "delta": {}, "finish_reason": finish}]})
    yield b"data: [DONE]\n\n"


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
    setattr(server, TOOL_ATTR, "")
    setattr(server, MODEL_ATTR, args.model)
    setattr(server, LOCK_ATTR, threading.Lock())

    sys.stdout.write(f"READY http={server.server_address[1]}\n")
    sys.stdout.flush()
    server.serve_forever()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
