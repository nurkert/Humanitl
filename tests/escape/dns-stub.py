#!/usr/bin/env python3
"""The recording name server of ESC-3 (HUM-115).

    dns-stub.py LOG PORTFILE

Binds UDP on 127.0.0.1 with a port the kernel picks (never 53: CI runners
often have systemd-resolved on 127.0.0.53:53), writes that port into PORTFILE
once it listens, and then writes one line per query into LOG:

    <epoch-ms> <qname> <qtype>

The name is lower case and without the trailing dot, the type is its mnemonic
(A, AAAA, ...) or TYPE<n>. Every query is answered with NXDOMAIN. A stub that
resolves nothing is all the proof needs: the question is only whether and when
the daemon asks, not what it gets. An answer of 127.0.0.1 would be refused as
a private address (ADR-006) and blur the case; NXDOMAIN ends the allowed
request as 502 with `reason: upstream_dns`, which is what ESC-3 expects.

run.sh points the daemon of the run at this stub with
HUMANITL_RESOLVER__NAMESERVER=127.0.0.1:<port>. The daemon then resolves
through `HickoryResolver`, which asks this server and nobody else, so the log
is the complete list of what the daemon resolved.

The line is written before the answer is sent: whoever reads the log after the
daemon has seen the answer finds the line in it.
"""

import os
import socket
import struct
import sys
import time

QTYPES = {
    1: "A",
    2: "NS",
    5: "CNAME",
    6: "SOA",
    12: "PTR",
    15: "MX",
    16: "TXT",
    28: "AAAA",
    33: "SRV",
    64: "SVCB",
    65: "HTTPS",
    255: "ANY",
}

HEADER = struct.Struct("!HHHHHH")
# QR (response) and RA (recursion available); RCODE 3 is NXDOMAIN.
FLAG_QR = 0x8000
FLAG_RA = 0x0080
MASK_OPCODE = 0x7800
FLAG_RD = 0x0100
RCODE_NXDOMAIN = 3


def question(packet):
    """(name, qtype, end of the question section) or None for garbage."""
    if len(packet) < HEADER.size:
        return None
    qdcount = HEADER.unpack_from(packet)[2]
    if qdcount < 1:
        return None
    labels = []
    pos = HEADER.size
    while True:
        if pos >= len(packet):
            return None
        length = packet[pos]
        pos += 1
        if length == 0:
            break
        # A client does not compress the name of its own question.
        if length & 0xC0:
            return None
        label = packet[pos : pos + length]
        if len(label) != length:
            return None
        labels.append(label.decode("ascii", "replace").lower())
        pos += length
    if pos + 4 > len(packet):
        return None
    qtype = struct.unpack_from("!H", packet, pos)[0]
    return ".".join(labels), qtype, pos + 4


def nxdomain(packet, end):
    """The answer: same id, opcode and RD, the question echoed, NXDOMAIN."""
    ident, flags = struct.unpack_from("!HH", packet)
    reply_flags = FLAG_QR | (flags & MASK_OPCODE) | (flags & FLAG_RD) | FLAG_RA | RCODE_NXDOMAIN
    return HEADER.pack(ident, reply_flags, 1, 0, 0, 0) + packet[HEADER.size : end]


def main():
    if len(sys.argv) != 3:
        print("usage: dns-stub.py LOG PORTFILE", file=sys.stderr)
        return 64
    log_path, port_path = sys.argv[1], sys.argv[2]
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(("127.0.0.1", 0))
    port = sock.getsockname()[1]
    with open(log_path, "a", encoding="ascii", errors="replace") as log:
        # Written only once the socket listens, and renamed into place, so
        # that run.sh never reads half a number.
        with open(port_path + ".tmp", "w", encoding="ascii") as handle:
            handle.write(f"{port}\n")
        os.replace(port_path + ".tmp", port_path)
        while True:
            packet, peer = sock.recvfrom(4096)
            asked = question(packet)
            if asked is None:
                continue
            name, qtype, end = asked
            kind = QTYPES.get(qtype, f"TYPE{qtype}")
            log.write(f"{int(time.time() * 1000)} {name} {kind}\n")
            log.flush()
            sock.sendto(nxdomain(packet, end), peer)


if __name__ == "__main__":
    sys.exit(main())
