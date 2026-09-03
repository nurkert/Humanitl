"""Der Body-Cap am laufenden Proxy, gemessen über seinen Unix-Socket.

Aufruf: ``python3 body_cap.py <proxy.sock> <cap>``. Geschickt werden zwei
Anfragen an denselben Host: eine mit einem Byte mehr als ``cap``, eine mit
genau ``cap`` Bytes. Ausgegeben wird eine Zeile, die ESC-4 prüft::

    over_cap=413/body_cap at_cap=504/timeout

Die Bytes auf der Leitung sind dieselben, die ``curl -x http://127.0.0.1:3128``
in der Sandbox schickt: Anfragezeile in absoluter Form, ``Host``-Kopfzeile,
``Content-Length``. Die Brücke des Shims ist ein Byte-Rohr, der Socket also
derselbe Weg — nur ohne Sandbox, die für eine Antwort des Proxys nichts
beiträgt.
"""

import socket
import sys

TIMEOUT_SECS = 30
HOST = "blocked.example"


def ask(socket_path: str, size: int, filler: bytes) -> str:
    """Schickt eine POST-Anfrage mit ``size`` Bytes und liest die Antwort.

    Zurück kommt ``<status>/<reason>`` aus der Statuszeile und der Zeile
    ``reason:`` des Block-Bodys (CONVENTIONS.md 3.5).
    """
    body = filler * size
    head = (
        f"POST http://{HOST}/upload HTTP/1.1\r\n"
        f"Host: {HOST}\r\n"
        "Content-Type: application/octet-stream\r\n"
        f"Content-Length: {len(body)}\r\n"
        "Connection: close\r\n\r\n"
    ).encode()

    handle = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    handle.settimeout(TIMEOUT_SECS)
    try:
        handle.connect(socket_path)
        handle.sendall(head + body)
        answer = b""
        while True:
            chunk = handle.recv(65536)
            if not chunk:
                break
            answer += chunk
    finally:
        handle.close()

    text = answer.decode("utf-8", "replace")
    fields = text.split(" ")
    status = fields[1] if text.startswith("HTTP/1.1 ") and len(fields) > 1 else "none"
    reason = "none"
    for line in text.splitlines():
        if line.startswith("reason: "):
            reason = line[len("reason: ") :].strip()
    return f"{status}/{reason}"


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: body_cap.py <proxy.sock> <cap>", file=sys.stderr)
        return 2
    socket_path = sys.argv[1]
    cap = int(sys.argv[2])
    over = ask(socket_path, cap + 1, b"x")
    at = ask(socket_path, cap, b"y")
    print(f"over_cap={over} at_cap={at}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
