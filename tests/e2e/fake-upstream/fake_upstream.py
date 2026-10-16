#!/usr/bin/env python3
"""Das Ziel des M2-Demolaufs (HUM-036).

Ein kleiner HTTP-Server, der unter einer Adresse gleichzeitig auf zwei Ports
lauscht und jede Anfrage mit einem JSON-Objekt beantwortet, das den Pfad, die
Methode, den `Host`-Kopf und die Länge des empfangenen Rumpfes zurückgibt:

* Klartext auf `--http-port` (im Demolauf 80),
* TLS auf `--https-port` (im Demolauf 443), mit dem Zertifikat aus
  `--cert`/`--key`.

Er steht damit für die drei Hosts `registry.npmjs.org`, `api.github.com` und
`evil.example` zugleich; welcher gemeint war, sagt der `Host`-Kopf, und der
Proxy findet die Adresse über `resolver.overrides`. Ein eigener Server je Host
wäre dieselbe Antwort dreimal.

Aufruf:

    fake_upstream.py --address ADRESSE [--http-port N] [--https-port N]
                     [--cert DATEI --key DATEI] [--capture DATEI]

Ohne `--cert`/`--key` lauscht nur der Klartext-Port. Sobald alle Listener
stehen, schreibt der Server eine Zeile `READY http=<port> https=<port|->` auf
stdout; danach geht dort nichts mehr hinaus. Das Zugriffsprotokoll läuft nach
stderr, eine Zeile je Anfrage im festen Format

    fake-upstream: <schema> <host> <methode> <pfad> <status> <rumpfbytes>

Mit `--capture` (HUM-055) hängt der Server je Anfrage zusätzlich eine Zeile
JSON an DATEI an: Schema, Methode, Pfad, alle Kopfzeilen (Namen klein
geschrieben, in der empfangenen Reihenfolge) und den Rumpf, als Text in UTF-8
(`body`, nicht dekodierbare Bytes ersetzt) und als Base64 (`body_b64`, Byte
für Byte), dazu seine Länge in Bytes. Ein Rumpf mit `Transfer-Encoding:
chunked` wird dafür vollständig gelesen und zusammengesetzt; dass er so kam,
steht in den Kopfzeilen. Das M4-Demo liest darin, was nach der Bearbeitung
wirklich beim Ziel ankam, und nicht nur, wie viel davon.

Das Demoskript zählt darin, welche Anfrage das Ziel wirklich erreicht hat. Die
Gegenprobe ist der Kern des Laufs: Was ein Mensch geblockt hat und was in die
Zeitüberschreitung lief, darf hier null Mal stehen.

Warum Python und kein Rust-Binary: Für ein Ziel, das nur zurückmeldet, wonach
gefragt wurde, wäre eine eigene Crate mehr Bauzeit als Nutzen. Der M1-Lauf hat
denselben Weg gewählt (`tests/e2e/fake_upstream.py`), und Python 3 liegt auf
Debian wie auf `ubuntu-latest` bereit. Die Abweichung von der Spezifikation
(dort ein axum-Server) steht in `backlog/CONVENTIONS.md` 4.22.
"""

import argparse
import base64
import json
import ssl
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

# Der Name des Schemas im Zugriffsprotokoll, je Listener gesetzt.
SCHEME_ATTR = "humanitl_scheme"

# Wohin `--capture` schreibt, am Server gesetzt; fehlt das Attribut, wird
# nichts mitgeschrieben.
CAPTURE_ATTR = "humanitl_capture"

# Schützt die Capture-Datei: Die Listener bedienen jede Anfrage in einem
# eigenen Thread.
CAPTURE_LOCK = threading.Lock()


class Echo(BaseHTTPRequestHandler):
    """Antwortet auf alles mit dem, wonach gefragt wurde."""

    # HTTP/1.1, damit der Proxy dieselbe Fassung spricht wie der Klient.
    protocol_version = "HTTP/1.1"
    server_version = "humanitl-fake-upstream/2"
    sys_version = ""

    def do_GET(self):  # noqa: N802 (Name kommt aus BaseHTTPRequestHandler)
        """Beantwortet eine GET-Anfrage."""
        self._echo(0)

    def do_HEAD(self):  # noqa: N802
        """Beantwortet eine HEAD-Anfrage; nur die Kopfzeilen gehen hinaus."""
        self._echo(0, body_out=False)

    def do_POST(self):  # noqa: N802
        """Beantwortet eine POST-Anfrage; der Rumpf wird gelesen und gezählt."""
        self._echo(self._read_body())

    def do_PUT(self):  # noqa: N802
        """Beantwortet eine PUT-Anfrage wie eine POST-Anfrage."""
        self._echo(self._read_body())

    def _read_body(self):
        """Liest den Rumpf vollständig und gibt seine Länge zurück."""
        if "chunked" in (self.headers.get("Transfer-Encoding") or "").lower():
            body = self._read_chunked()
        else:
            length = int(self.headers.get("Content-Length") or 0)
            body = self.rfile.read(length) if length else b""
        self._capture(body)
        return len(body)

    def _read_chunked(self):
        """Setzt einen Rumpf mit `Transfer-Encoding: chunked` zusammen."""
        parts = []
        while True:
            size_line = self.rfile.readline().split(b";", 1)[0].strip()
            size = int(size_line or b"0", 16)
            if size == 0:
                # Trailer bis zur leeren Zeile überlesen.
                while self.rfile.readline() not in (b"\r\n", b"\n", b""):
                    pass
                return b"".join(parts)
            parts.append(self.rfile.read(size))
            self.rfile.readline()

    def _capture(self, body):
        """Hängt die Anfrage samt Rumpf an die Capture-Datei an (HUM-055)."""
        target = getattr(self.server, CAPTURE_ATTR, None)
        if not target:
            return
        record = {
            "scheme": getattr(self.server, SCHEME_ATTR, "http"),
            "method": self.command,
            "path": self.path,
            "headers": [[name.lower(), value] for name, value in self.headers.items()],
            "body": body.decode("utf-8", "replace"),
            "body_b64": base64.b64encode(body).decode("ascii"),
            "body_bytes": len(body),
        }
        line = json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n"
        with CAPTURE_LOCK:
            with open(target, "a", encoding="utf-8") as handle:
                handle.write(line)

    def _echo(self, request_bytes, body_out=True):
        """Schreibt die Antwort und protokolliert die Anfrage."""
        body = json.dumps(
            {
                "path": self.path,
                "method": self.command,
                "host": self.headers.get("Host", ""),
                "request_bytes": request_bytes,
            }
        ).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if body_out:
            self.wfile.write(body)
        self._access(200, request_bytes)

    def _access(self, status, request_bytes):
        """Eine Zeile des Zugriffsprotokolls, im festen Format."""
        scheme = getattr(self.server, SCHEME_ATTR, "http")
        sys.stderr.write(
            "fake-upstream: {} {} {} {} {} {}\n".format(
                scheme,
                self.headers.get("Host", "-"),
                self.command,
                self.path,
                status,
                request_bytes,
            )
        )
        sys.stderr.flush()

    def log_message(self, fmt, *args):
        """Unterdrückt das eingebaute Protokoll; `_access` schreibt es selbst."""

    def log_error(self, fmt, *args):
        """Fehler des Servers gehören nach stderr, aber ohne den Zeitstempel."""
        sys.stderr.write("fake-upstream: error " + (fmt % args) + "\n")
        sys.stderr.flush()


def make_server(address, port, scheme, context=None, capture=None):
    """Bindet einen Listener und gibt ihn zurück, noch ohne zu bedienen."""
    server = ThreadingHTTPServer((address, port), Echo)
    setattr(server, SCHEME_ATTR, scheme)
    setattr(server, CAPTURE_ATTR, capture)
    if context is not None:
        server.socket = context.wrap_socket(server.socket, server_side=True)
    return server


def main(argv):
    """Startet die Listener und meldet, sobald sie stehen."""
    parser = argparse.ArgumentParser(description="the target of the M2 demo run")
    parser.add_argument("--address", required=True)
    parser.add_argument("--http-port", type=int, default=80)
    parser.add_argument("--https-port", type=int, default=443)
    parser.add_argument("--cert")
    parser.add_argument("--key")
    parser.add_argument("--capture")
    args = parser.parse_args(argv[1:])

    servers = [make_server(args.address, args.http_port, "http", capture=args.capture)]
    https_port = "-"
    if args.cert and args.key:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(args.cert, args.key)
        servers.append(
            make_server(args.address, args.https_port, "https", context, args.capture)
        )
        https_port = str(args.https_port)

    for server in servers[1:]:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()

    sys.stdout.write(f"READY http={args.http_port} https={https_port}\n")
    sys.stdout.flush()
    servers[0].serve_forever()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
