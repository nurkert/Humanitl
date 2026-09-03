#!/usr/bin/env python3
"""Das Ziel, das der M1-Demolauf freigibt (HUM-021).

Ein winziger HTTP-Server, der jede Anfrage mit einem JSON-Objekt beantwortet,
das den angefragten Pfad, die Methode und den `Host`-Kopf zurückgibt. Das
Demoskript prüft daran, dass eine erlaubte Anfrage wirklich beim Ziel ankommt
und dass die Antwort des Ziels unverändert beim Klienten in der Sandbox landet.

Aufruf: `fake_upstream.py <adresse> [port]`. Ohne Port sucht das Betriebssystem
einen freien; die erste Zeile auf stdout lautet dann `PORT <nummer>`, und das
Skript liest sie. Danach schreibt der Server nur noch sein Zugriffsprotokoll
nach stderr.

Warum kein Rust-Binary: `humanitl-fake-upstream` aus der Spezifikation gibt es
noch nicht, und für ein Ziel, das nur seinen eigenen Pfad zurückmeldet, wäre
eine eigene Crate mehr Bauzeit als Nutzen. Python 3 liegt auf Debian und auf
`ubuntu-latest` ohnehin bereit.
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Echo(BaseHTTPRequestHandler):
    """Antwortet auf alles mit dem, was gefragt wurde."""

    # HTTP/1.1, damit der Proxy dieselbe Fassung spricht wie der Klient.
    protocol_version = "HTTP/1.1"
    server_version = "humanitl-fake-upstream/0"
    sys_version = ""

    def do_GET(self):  # noqa: N802 (Name kommt aus BaseHTTPRequestHandler)
        """Beantwortet eine GET-Anfrage."""
        self._echo()

    def do_POST(self):  # noqa: N802
        """Beantwortet eine POST-Anfrage; der Body wird gelesen und verworfen."""
        length = int(self.headers.get("Content-Length") or 0)
        if length:
            self.rfile.read(length)
        self._echo()

    def _echo(self):
        body = json.dumps(
            {
                "path": self.path,
                "method": self.command,
                "host": self.headers.get("Host", ""),
            }
        ).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, fmt, *args):
        """Schreibt das Zugriffsprotokoll nach stderr, nie nach stdout."""
        sys.stderr.write("fake-upstream: " + (fmt % args) + "\n")


def main(argv):
    """Startet den Server und meldet den gewählten Port."""
    if not 2 <= len(argv) <= 3:
        sys.stderr.write("usage: fake_upstream.py <address> [port]\n")
        return 2
    address = argv[1]
    port = int(argv[2]) if len(argv) == 3 else 0
    server = ThreadingHTTPServer((address, port), Echo)
    sys.stdout.write(f"PORT {server.server_address[1]}\n")
    sys.stdout.flush()
    server.serve_forever()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
