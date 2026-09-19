#!/usr/bin/env sh
# Paritäts-Prüfung (HUM-078, ADR-018): Jede RPC des Vertrags hat ein
# Unterkommando der Kommandozeile oder eine begründete Ausnahme, und die
# Tabelle `docs/reference/parity.md` auf der Platte ist die, die aus dem Code
# entsteht.
#
# `cargo xtask docs --check` erzeugt die Tabelle im Speicher und schreibt
# nichts. Exit 1, wenn
#   - eine RPC weder eine Zeile in `PARITY` (`daemon/bin/humanitl/src/parity.rs`)
#     noch einen Eintrag in `daemon/xtask/parity_exempt.toml` hat,
#   - ein Eintrag eine RPC nennt, die der Vertrag nicht kennt, eine Ausnahme
#     keine Begründung hat oder ein Ort der UI-Registry keine Datei,
#   - eine Zeile der CLI-Tabelle oder der UI-Registry nicht im strengen Format
#     steht,
#   - die Datei fehlt oder von der erzeugten abweicht.
# RPCs ohne Ort in der Oberfläche sind nur Warnungen.
#
# In der CI ist die Datei auf der Platte die eingecheckte. Ein Vergleich über
# die Versionsverwaltung ist deshalb nicht nötig, und dieselbe Prüfung läuft
# lokal in `make check`, auch in einem Arbeitsbaum mit offenen Änderungen.
#
# Vorher laufen die Tests in `daemon/bin/humanitl/src/parity.rs`: Der Generator
# liest `PARITY` nur als Text und sieht nicht, ob ein Eintrag ein Unterkommando
# der `clap`-Struktur nennt. Ohne diesen Schritt ginge ein Eintrag wie
# `flows watch`, den es nicht gibt, durch diesen Job.
#
# Was diese Prüfung nicht sieht: Parität auf Feld-Ebene (ob ein Unterkommando
# alle Felder seiner RPC anbietet) und Fähigkeiten, die nur in einem Client
# stehen und gar keine RPC haben.
set -eu
cd "$(dirname "$0")/../.."

cd daemon
cargo test -p humanitl --bin humanitl parity::
exec cargo xtask docs --check
