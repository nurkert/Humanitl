#!/usr/bin/env sh
# Nennt die Tests, die in einem Lauf von `cargo test` gefallen sind, als
# GitHub-Annotationen (HUM-133).
#
#   scripts/ci/test-report.sh LOGDATEI     schreibt je gefallenem Test eine Zeile
#   scripts/ci/test-report.sh --self-test  prüft den Auswerter selbst
#
# **Wozu.** Der Schritt `rust-test` fährt rund neunhundert Tests über ein
# Dutzend Binärdateien. Fällt einer, steht in der Annotation des Laufs genau
# ein Satz: `Process completed with exit code 2`. Das Job-Log verlangt
# Schreibrechte am Repository und ist von außen nicht zu lesen (403), also
# weiß niemand, welcher Test fiel — und ein roter Lauf, dessen Grund niemand
# lesen kann, ist eine Sprint-Bedingung, die niemand erfüllen kann
# (`BACKLOG.md` 8). Am 2026-09-06 ist das viermal passiert.
#
# **Was es liest.** `cargo test` schreibt je Binärdatei zwei Blöcke unter der
# Überschrift `failures:` — erst die Meldungen (`---- name stdout ----`), dann
# die Namen, eingerückt, und darunter die Ergebniszeile:
#
#   failures:
#
#       modul::test_eins
#       modul::test_zwei
#
#   test result: FAILED. 1 passed; 2 failed; ...
#
# Gesammelt wird je Block, hinausgegeben erst an der Ergebniszeile; warum,
# steht im Auswerter selbst.
#
# GitHub zeigt höchstens zehn Annotationen je Schritt; ab dem elften Namen ist
# die zehnte Zeile die, die den Rest sammelt.
#
# Exit 0 immer, außer beim Selbsttest: Dieses Skript berichtet, es urteilt
# nicht. Der Exit-Code des Laufs gehört `cargo test`.
set -eu

MAX_ANNOTATIONS=10

if ! command -v python3 >/dev/null 2>&1; then
  echo "test-report: python3 is required" >&2
  exit 1
fi

report() {
  python3 - "$1" "$MAX_ANNOTATIONS" <<'PYTHON'
import re
import sys
from pathlib import Path

log = Path(sys.argv[1])
limit = int(sys.argv[2])
if not log.exists():
    print(f"::error::test-report: {log} does not exist")
    raise SystemExit(0)

text = log.read_text(errors="replace")

# Gesammelt wird je Block, hinausgegeben erst an seiner Ergebniszeile.
#
# **Warum nicht Zeile fuer Zeile.** In den Block schreiben andere hinein: Der
# Schritt fuehrt `2>&1` zusammen, und was ein Kindprozess der Tests auf
# `stderr` sagt -- `bwrap: setting up uid map: Permission denied` zum
# Beispiel --, steht mitten zwischen den Namen und ohne Einrueckung. Wer den
# Block daran enden liesse, verloere jeden Namen dahinter; und gerade der Lauf,
# in dem so etwas steht, ist der, dessen Namen jemand lesen will.
#
# Andersherum kann eine Panik-Meldung selbst eine Zeile `failures:` in Spalte 0
# tragen. Ein Block, der nie an einer Ergebniszeile ankommt, wird deshalb
# verworfen, statt seine Zeilen fuer Namen zu halten.
# Ein Name, wie `libtest` ihn schreibt: genau vier Leerzeichen, dann ein Wort
# -- oder die Form eines Doku-Tests, `datei.rs - pfad (line 3)`. Der Pfad darf
# dabei fehlen: Ein Doku-Test in den `//!`-Zeilen einer Datei heisst
# `src/lib.rs - (line 3)`, und diese Crates haben solche.
#
# **Einrueckung allein reicht nicht.** Ein Panik-Bericht, den `libtest` nicht
# eingefangen hat -- eine Panik in einem Arbeitsfaden, mit `RUST_BACKTRACE=1`
# in der CI --, steht mit eingerueckten Rahmen mitten in der Namensliste:
# `   0: rust_begin_unwind` und `             at /rustc/.../panicking.rs:665`.
# Wer jede eingerueckte Zeile fuer einen Namen haelt, erfindet drei
# Annotationen je Panik und drueckt die echten Namen in die Sammelzeile. Und
# die Statuszeilen von Cargo selbst (`     Running unittests ...`,
# `   Doc-tests ...`) sind ebenfalls eingerueckt.
NAME = re.compile(r" {4}(\S+|\S+ - .*\(line \d+\))$")

names = []
candidates = []
in_block = False
last_was_candidate = False


def keep(found):
    for name in found:
        if name not in names:
            names.append(name)


for line in text.splitlines():
    if line == "failures:":
        in_block = True
        candidates = []
        continue
    if not in_block:
        continue
    stripped = line.strip()
    if not stripped:
        # Auch eine leere Zeile beendet den Namen davor: Endet die Datei mit
        # der Einrueckung des naechsten, noch nicht geschriebenen Namens, waere
        # der davor sonst das Bruchstueck, das unten entfaellt.
        last_was_candidate = False
        continue
    if stripped.startswith("test result:"):
        keep(candidates)
        in_block = False
        candidates = []
        continue
    # `---- name stdout ----` in Spalte 0 beginnt die Meldungen: Dieser Block
    # war keiner mit Namen. **Nur in Spalte 0**, und nur diese eine Marke: Ein
    # Kindprozess, der `error:` oder einen Strich schreibt, waere sonst das
    # Ende eines Blocks, dessen Namen schon gesammelt sind -- und die waeren
    # dann weg. Ein Block, der faelschlich offen ist, stirbt ohnehin daran,
    # dass er nie an einer Ergebniszeile ankommt.
    if line.startswith("----"):
        in_block = False
        candidates = []
        continue
    if NAME.match(line):
        candidates.append(stripped)
        last_was_candidate = True
        continue
    # Alles andere in Spalte 0 ist Laerm eines Kindprozesses: ueberlesen, aber
    # den Block offen lassen.
    last_was_candidate = False

# Ein Lauf, den jemand abgeschnitten hat, endet mitten im Block. Was bis dahin
# steht, ist brauchbar -- bis auf die letzte Zeile, wenn die Datei ohne
# Zeilenende aufhoert: Die ist ein Bruchstueck und kein Name.
if in_block and candidates:
    # Nur wenn die letzte Zeile selbst ein Name war: Endet die Datei mitten in
    # einer Zeile eines Kindprozesses, gehoert das Bruchstueck nicht der Liste,
    # und ein `pop` naehme einen ganzen Namen mit.
    if not text.endswith("\n") and last_was_candidate:
        candidates.pop()
    keep(candidates)

if not names:
    print("::error::test-report: the step failed, but no test named itself; read the artifact")
    raise SystemExit(0)

# Zehn Annotationen zeigt GitHub je Schritt, und die elfte fiele weg -- also
# ist die Sammelzeile die zehnte, wenn es eine braucht.
shown = names[: limit - 1] if len(names) > limit else names
for name in shown:
    print(f"::error::rust-test failed: {name}")
if len(shown) < len(names):
    rest = ", ".join(names[len(shown):])
    print(f"::error::rust-test: {len(names) - len(shown)} more failed: {rest}")
PYTHON
}

# expect_line TEXT MUSTER MELDUNG
expect_line() {
  if ! printf '%s\n' "$1" | grep -q "$2"; then
    echo "self-test: $3" >&2
    printf '%s\n' "$1" >&2
    exit 1
  fi
}

self_test() {
  work=$(mktemp -d)
  # shellcheck disable=SC2064 # der Pfad steht jetzt fest, nicht erst beim Ende.
  trap "rm -rf '$work'" EXIT

  # 1. Zwei gefallene Tests, mit dem Meldungsblock davor.
  cat >"$work/two.log" <<'LOG'
running 3 tests
test cmd::run::tests::ok ... ok
test cmd::run::tests::first ... FAILED
test cmd::run::tests::second ... FAILED

failures:

---- cmd::run::tests::first stdout ----
thread 'first' panicked at src/lib.rs:1:1:
assertion failed

failures:
    cmd::run::tests::first
    cmd::run::tests::second

test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
LOG

  # 2. Lärm eines Kindprozesses mitten in der Namensliste.
  cat >"$work/noise.log" <<'LOG'
failures:
    a::one
bwrap: setting up uid map: Permission denied
    a::two
    a::three

test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out
LOG

  # 3. Eine Panik-Meldung, die selbst `failures:` in Spalte 0 trägt.
  cat >"$work/panic.log" <<'LOG'
failures:

---- a::real stdout ----
thread 'a::real' panicked at src/lib.rs:1:1:
failures:
left: "x"
right: ""

failures:
    a::real

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
LOG

  # 4. Ein Lauf, den jemand abgeschnitten hat.
  printf 'failures:\n    a::one\n    a::tw' >"$work/cut.log"

  # 4b. Abgeschnitten mitten in der Zeile eines Kindprozesses: Der Name davor
  # bleibt.
  printf 'failures:\n    a::one\n    a::two\nbwrap: killed' >"$work/cut2.log"

  # 4d. Abgeschnitten nach einem ganzen Namen, mit der Einrückung des nächsten.
  printf 'failures:\n    a::one\n    ' >"$work/cut3.log"

  # 4c. Lärm, der aussieht wie eine Marke des Testläufers.
  cat >"$work/marks.log" <<'LOG'
failures:
    a::one
error: connection reset by peer
    a::two
warning: something happened
    a::three

test result: FAILED. 0 passed; 3 failed; 0 ignored
LOG

  # 4e. Ein Panik-Bericht, den `libtest` nicht eingefangen hat, mitten in der
  # Namensliste: eingerückte Rahmen, die keine Namen sind.
  cat >"$work/backtrace.log" <<'LOG'
failures:
    proxy::tests::hold_then_allow
thread '<unnamed>' panicked at crates/proxy/src/egress.rs:77:14:
stack backtrace:
   0: rust_begin_unwind
             at /rustc/abc/library/std/src/panicking.rs:665:5
   1: core::panicking::panic_fmt
    proxy::tests::block_note_is_sanitised

test result: FAILED. 8 passed; 2 failed; 0 ignored
LOG

  # 4f. Ein verirrtes `failures:` vor einer grünen Binärdatei: Cargos eigene
  # Statuszeilen sind eingerückt und trotzdem keine Namen.
  cat >"$work/stray.log" <<'LOG'
failures:
     Running unittests src/main.rs (target/debug/deps/crate_b-2)
   Doc-tests humanitl-core

test result: ok. 1 passed; 0 failed; 0 ignored
LOG

  # 4g. Doku-Tests, die fallen: mit Pfad, mit Generics und ohne Pfad -- der
  # letzte heißt `src/lib.rs - (line 3)`, ohne Leerzeichen vor der Klammer.
  cat >"$work/doc.log" <<'LOG'
failures:
    src/lib.rs - foo::bar (line 3)
    src/lib.rs - Map<K,V>::insert (line 18)
    src/lib.rs - (line 3)

test result: FAILED. 0 passed; 3 failed; 0 ignored
LOG

  # 5. Ein Lauf ohne einen einzigen gefallenen Test.
  cat >"$work/green.log" <<'LOG'
running 2 tests
test a ... ok
test b ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
LOG

  out=$(report "$work/two.log")
  expect_line "$out" "^::error::rust-test failed: cmd::run::tests::first$" \
    "the first name is missing"
  expect_line "$out" "^::error::rust-test failed: cmd::run::tests::second$" \
    "the second name is missing"
  # `|| true`: `grep -c` endet mit 1, wenn es nichts findet, und unter `set -e`
  # stürbe das Skript vor der Meldung, die den Grund nennt.
  count=$(printf '%s\n' "$out" | grep -c "cmd::run::tests::first" || true)
  [ "$count" = "1" ] || {
    echo "self-test: the first name appears $count times, not once" >&2
    exit 1
  }

  noise=$(report "$work/noise.log")
  for name in one two three; do
    expect_line "$noise" "^::error::rust-test failed: a::$name$" \
      "a line of a child process swallowed the names after it"
  done

  panic=$(report "$work/panic.log")
  expect_line "$panic" "^::error::rust-test failed: a::real$" "the real name is missing"
  lines=$(printf '%s\n' "$panic" | wc -l)
  [ "$lines" = "1" ] || {
    echo "self-test: a panic body became $lines annotations, not 1" >&2
    printf '%s\n' "$panic" >&2
    exit 1
  }

  cut=$(report "$work/cut.log")
  expect_line "$cut" "^::error::rust-test failed: a::one$" "the whole name is missing"
  if printf '%s\n' "$cut" | grep -q "a::tw$"; then
    echo "self-test: the cut fragment became a name" >&2
    printf '%s\n' "$cut" >&2
    exit 1
  fi

  cut2=$(report "$work/cut2.log")
  for name in one two; do
    expect_line "$cut2" "^::error::rust-test failed: a::$name$" \
      "a cut line of a child process took a whole name with it"
  done

  cut3=$(report "$work/cut3.log")
  expect_line "$cut3" "^::error::rust-test failed: a::one$" \
    "a trailing indentation took the whole name before it"

  marks=$(report "$work/marks.log")
  for name in one two three; do
    expect_line "$marks" "^::error::rust-test failed: a::$name$" \
      "a child line that looks like a marker discarded the names before it"
  done

  # 6. Zwölf gefallene Tests: zehn Zeilen, die zehnte sammelt den Rest.
  {
    echo "failures:"
    for n in 01 02 03 04 05 06 07 08 09 10 11 12; do
      echo "    modul::test_$n"
    done
    echo "test result: FAILED. 0 passed; 12 failed; 0 ignored"
  } >"$work/many.log"
  many=$(report "$work/many.log")
  lines=$(printf '%s\n' "$many" | wc -l)
  [ "$lines" = "10" ] || {
    echo "self-test: twelve failures gave $lines annotations, not 10" >&2
    printf '%s\n' "$many" >&2
    exit 1
  }
  expect_line "$many" "3 more failed: modul::test_10, modul::test_11, modul::test_12" \
    "the collecting line does not name the rest"

  backtrace=$(report "$work/backtrace.log")
  lines=$(printf '%s\n' "$backtrace" | wc -l)
  [ "$lines" = "2" ] || {
    echo "self-test: an uncaught backtrace became $lines annotations, not 2" >&2
    printf '%s\n' "$backtrace" >&2
    exit 1
  }
  expect_line "$backtrace" "^::error::rust-test failed: proxy::tests::hold_then_allow$" \
    "the name before the backtrace is missing"
  expect_line "$backtrace" "^::error::rust-test failed: proxy::tests::block_note_is_sanitised$" \
    "the name after the backtrace is missing"

  stray=$(report "$work/stray.log")
  expect_line "$stray" "no test named itself" \
    "a status line of cargo became a test name"

  doc=$(report "$work/doc.log")
  expect_line "$doc" "^::error::rust-test failed: src/lib.rs - foo::bar (line 3)$" \
    "the name of a doc test is missing"
  expect_line "$doc" "^::error::rust-test failed: src/lib.rs - Map<K,V>::insert (line 18)$" \
    "a doc test with generics is missing"
  expect_line "$doc" "^::error::rust-test failed: src/lib.rs - (line 3)$" \
    "a doc test of the crate docs, whose name has no path, is missing"

  green=$(report "$work/green.log")
  expect_line "$green" "no test named itself" "a green log should not name a test"

  # 7. Eine Datei, die es nicht gibt.
  missing=$(report "$work/there-is-no-such-file.log")
  expect_line "$missing" "does not exist" "a missing log should say so"

  echo "test-report: self-test ok (13 cases)"
}

case "${1:---help}" in
  --self-test)
    self_test
    ;;
  --help | -h)
    sed -n '2,6p' "$0"
    ;;
  *)
    report "$1"
    ;;
esac
