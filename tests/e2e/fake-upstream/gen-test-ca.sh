#!/usr/bin/env sh
# Erzeugt die Testzertifikate des M2-Demolaufs (HUM-036).
#
#   gen-test-ca.sh VERZEICHNIS HOST [HOST...]
#
# Legt in VERZEICHNIS an:
#
#   test-ca.crt   die Wurzel, die `resolver.test_ca` benennt
#   test-ca.key   ihr Schlüssel, nur für diesen Lauf
#   upstream.crt  das Blatt für alle genannten Hosts (SAN je Host)
#   upstream.key  sein Schlüssel
#
# Nichts davon liegt im Repository. Das Material entsteht bei jedem Lauf neu,
# lebt im Wegwerf-Baum unter /tmp und verschwindet mit ihm. Ein Testschlüssel,
# der eingecheckt wäre, wäre ein Schlüssel, den jeder hat.
#
# Der Daemon nimmt diese Wurzel nur an, wenn `resolver.test_ca` auf sie zeigt
# **und** er ausdrücklich mit `--allow-test-ca` gestartet wurde. Beides ist
# Absicht: Ein Testzertifikat, das ohne Flag gälte, wäre ein Loch in der
# Sicherheitsaussage von `docs/SECURITY.md`. Solange der Daemon das Flag noch
# nicht kennt (Stand HUM-036, siehe `backlog/CONVENTIONS.md` 4.22), belegt der
# Demolauf mit diesem Material die andere Richtung: dass eine fremde Wurzel in
# der Konfiguration allein nichts bewirkt.
set -eu

# Zuerst die Maske, dann die erste Datei. Ein `chmod` am Ende deckt nur den
# Fall ab, in dem das Skript bis dorthin kommt; bricht `openssl` vorher ab,
# liegt ein privater Schlüssel mit den Vorgaberechten (üblicherweise 0644) im
# Verzeichnis. Dass es ein Testschlüssel ist, ändert nichts an der Gewohnheit,
# und der Lauf beschreibt genau dieses Verzeichnis als sicher.
umask 077

if [ "$#" -lt 2 ]; then
    echo "usage: gen-test-ca.sh DIR HOST [HOST...]" >&2
    exit 2
fi

dir="$1"
shift
mkdir -p "$dir"

# Die SAN-Liste des Blattes: ein `DNS:`-Eintrag je Host.
san=""
for host in "$@"; do
    if [ -z "$san" ]; then
        san="DNS:$host"
    else
        san="$san,DNS:$host"
    fi
done

# Die Wurzel. `-noenc` statt `-nodes`: derselbe Schalter, aber der Name, den
# OpenSSL 3 in seiner Hilfe führt; ältere Fassungen verstehen ihn seit 3.0.
openssl req -x509 -newkey rsa:2048 -noenc -days 2 \
    -keyout "$dir/test-ca.key" -out "$dir/test-ca.crt" \
    -subj "/CN=Humanitl e2e test CA" \
    -addext "basicConstraints=critical,CA:TRUE,pathlen:0" \
    -addext "keyUsage=critical,keyCertSign,cRLSign" \
    > /dev/null 2>&1

# Das Blatt, von der Wurzel unterschrieben, mit einem SAN je Host.
openssl req -newkey rsa:2048 -noenc \
    -keyout "$dir/upstream.key" -out "$dir/upstream.csr" \
    -subj "/CN=$1" \
    > /dev/null 2>&1

openssl x509 -req -in "$dir/upstream.csr" -days 2 \
    -CA "$dir/test-ca.crt" -CAkey "$dir/test-ca.key" -CAcreateserial \
    -out "$dir/upstream.crt" \
    -extfile /dev/stdin > /dev/null 2>&1 <<EOF
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
subjectAltName=$san
EOF

rm -f "$dir/upstream.csr"
# Die Zertifikate darf lesen, wer will; die Schlüssel bleiben bei dem, was
# `umask 077` ihnen von Anfang an gegeben hat.
chmod 644 "$dir/test-ca.crt" "$dir/upstream.crt"
