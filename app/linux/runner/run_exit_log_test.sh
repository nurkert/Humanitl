#!/usr/bin/env bash
# Übersetzt und startet den Test des Runner-Protokolls (HUM-136).
#
# `flutter test` kennt nur Dart, und `flutter build linux` übersetzt nur das
# Ziel `runner`. Dieser Test ist C++ und braucht deshalb seinen eigenen
# Aufruf. Er hängt an nichts außer POSIX: kein GTK, kein Flutter, keine
# Ephemeral-Header, also läuft er überall, wo ein C++-Übersetzer steht.
#
# In `make check` hängt er über das Ziel `runner-test`. Dreiundzwanzig
# Zusicherungen, die niemand fährt, sind dreiundzwanzig Zusicherungen, die
# verrotten.
set -euo pipefail
cd "$(dirname "$0")"

out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT

: "${CXX:=g++}"
"$CXX" -std=c++17 -Wall -Wextra -Werror -O1 -pthread \
  exit_log.cc exit_log_test.cc -o "$out/exit_log_test"
"$out/exit_log_test"
