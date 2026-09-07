#!/usr/bin/env sh
# Nennt die Tests, die in einem Lauf von `cargo test` gefallen sind, als
# GitHub-Annotationen (HUM-133).
#
#   scripts/ci/test-report.sh LOGDATEI     schreibt je gefallenem Test eine Zeile
#                                          mit seinem Namen und, wo eine dasteht,
#                                          der Zeile seiner Zusicherung
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

# Und der Grund, wo einer dasteht.
#
# **Warum das hier steht.** Ein Name allein sagt, welcher Test fiel, aber nicht,
# woran; wer den Grund braucht, muss das Artefakt herunterladen, und das
# verlangt Anmeldung. Am 2026-09-07 sind zwei Läufe an `one_writer_many_readers`
# gescheitert, und zwölf Läufe derselben Datei auf der Entwicklermaschine waren
# grün -- ohne die Zeile der Zusicherung ist so ein Fall nicht zu greifen.
#
# `libtest` schreibt die Meldungen in denselben `failures:`-Abschnitt, vor die
# Namen: `---- <name> stdout ----`, darunter die Panik. Genommen wird der Ort
# aus der `panicked at`-Zeile und die Zeile darunter, denn dort steht die
# Meldung der Zusicherung; Rahmen eines Backtrace und der `note:`-Hinweis
# bleiben draußen.
# **Verankert und mit Faden.** `libtest` schreibt vor der Panik alles, was der
# Test selbst ausgegeben hat. Ein Test, dessen Ausgabe die Worte `panicked at`
# trägt -- ein Log, das er zitiert, eine Meldung, die er prüft --, lieferte
# sonst den ersten Treffer, und die Annotation nennte eine Datei, die nie lief
# (gemessen an echtem `cargo test`). Der Faden gehört dazu, weil in denselben
# Block auch Kindprozesse schreiben: Die Panik des Tests selbst gilt vor jeder
# fremden.
PANIC = re.compile(r"^thread '(?P<thread>.*?)' (?:\(\d+\) )?panicked at (?P<where>[^\s].*):$")
HEAD = re.compile(r"^---- (.+) stdout ----$")
# Was nicht mehr zu einer Panik gehört. Ohne diese Grenzen nähme eine Panik
# ohne eigene Meldung die nächste Überschrift als ihre Meldung.
ENDS = ("note:", "stack backtrace", "----", "failures:", "test result:")

reasons = {}
# Ob der Grund vom Faden des Tests selbst stammt; eine fremde Panik weicht ihr.
from_own_thread = {}
current = None
lines = text.splitlines()


def belongs_to(thread, test):
    """Wahr, wenn dieser Faden der Test selbst ist."""
    return thread == test or test.endswith(f"::{thread}")


def message_after(start):
    """Die Meldung der Zusicherung, mit ihren Werten.

    `assert_eq!` schreibt die Werte eingerückt unter die erste Zeile, und
    gerade sie sind bei einem Fehlschlag, den niemand nachstellen kann, das
    Entscheidende.
    """
    parts = []
    for following in lines[start:]:
        stripped = following.strip()
        if not stripped:
            if parts:
                break
            continue
        if stripped.startswith(ENDS) or PANIC.match(following):
            break
        if not parts:
            parts.append(stripped)
            continue
        # Nur die eingerückte Fortsetzung derselben Meldung.
        if following[:1].isspace():
            parts.append(stripped)
            continue
        break
    return " ".join(parts)


for index, line in enumerate(lines):
    head = HEAD.match(line)
    if head:
        current = head.group(1).strip()
        continue
    if current is None:
        continue
    # Nur die Marken, die einen Abschnitt wirklich beenden. **Nicht jede Zeile
    # mit Strichen**: Ein Test, der `---- setup ----` ausgibt, verlöre sonst
    # seinen eigenen Grund, weil der Block ab da niemandem mehr gehörte. Den
    # nächsten Block fängt `HEAD` eine Zeile höher ab.
    if line == "failures:" or line.startswith("test result:"):
        current = None
        continue
    panic = PANIC.match(line)
    if not panic:
        continue
    own = belongs_to(panic.group("thread"), current)
    # Die erste Panik gewinnt, außer eine spätere gehört dem Test selbst und
    # die bisherige einem fremden Faden.
    if current in reasons and (from_own_thread[current] or not own):
        continue
    where = panic.group("where")
    message = message_after(index + 1)
    reasons[current] = f"{where}: {message}" if message else where
    from_own_thread[current] = own


def annotate(name):
    reason = reasons.get(name)
    if not reason:
        return f"rust-test failed: {name}"
    # Lang genug für eine Zusicherung, kurz genug für eine Annotation.
    if len(reason) > 300:
        reason = f"{reason[:297]}..."
    return f"rust-test failed: {name} -- {reason}"

# Zehn Annotationen zeigt GitHub je Schritt, und die elfte fiele weg -- also
# ist die Sammelzeile die zehnte, wenn es eine braucht.
shown = names[: limit - 1] if len(names) > limit else names
for name in shown:
    print(f"::error::{annotate(name)}")
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

  # 4h. Eine Panik mit Rahmen und Hinweis: Genommen wird die Zeile der
  # Zusicherung, nicht der `note:`-Hinweis und kein Rahmen.
  cat >"$work/reason.log" <<'LOG'
running 2 tests
test a::loud ... FAILED
test a::quiet ... FAILED

failures:

---- a::loud stdout ----

thread 'a::loud' (4711) panicked at crates/ipc/tests/terminal.rs:795:5:
the reader gets the scrollback: ""
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
stack backtrace:
   0: rust_begin_unwind

---- a::quiet stdout ----

thread 'a::quiet' (4712) panicked at src/lib.rs:9:9:
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

failures:

    a::loud
    a::quiet

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
LOG

  # 4i. Ein Test, der vor seiner Panik selbst etwas ausgibt -- auch eine Zeile
  # mit Strichen. Sein Grund bleibt seiner.
  cat >"$work/chatty.log" <<'LOG'
running 1 test
test a::chatty ... FAILED

failures:

---- a::chatty stdout ----
---- setup ----
the fixture is ready
thread 'a::chatty' panicked at src/lib.rs:7:7:
the door stayed open
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

failures:

    a::chatty

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
LOG

  # 4j. Ein Test, dessen eigene Ausgabe wie eine Panik aussieht: Der Grund ist
  # seine Panik und nicht sein Zitat.
  cat >"$work/quoting.log" <<'LOG'
failures:

---- a::quoting stdout ----
thread 'other' panicked at /tmp/quoted.rs:1:1:
this line is only my own stdout
thread 'a::quoting' panicked at src/lib.rs:26:9:
the door stayed open

failures:

    a::quoting

test result: FAILED. 0 passed; 1 failed; 0 ignored
LOG

  # 4k. `assert_eq!`: Die Werte stehen eingerückt darunter und gehören zum
  # Grund -- bei einem Fehlschlag, den niemand nachstellen kann, sind sie das
  # Entscheidende.
  cat >"$work/values.log" <<'LOG'
failures:

---- a::values stdout ----
thread 'a::values' panicked at src/lib.rs:7:9:
assertion `left == right` failed: the reader gets the scrollback
  left: [1, 2, 3]
 right: [1, 9, 3]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

failures:

    a::values

test result: FAILED. 0 passed; 1 failed; 0 ignored
LOG

  # 4l. Zwei Paniken in einem Block, die erste ohne Meldung: Die erste gilt,
  # und die zweite wird nicht ihre Meldung.
  cat >"$work/twice.log" <<'LOG'
failures:

---- a::twice stdout ----
thread 'a::twice' panicked at src/first.rs:1:1:
thread 'a::twice' panicked at src/second.rs:2:2:
the second message
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

failures:

    a::twice

test result: FAILED. 0 passed; 1 failed; 0 ignored
LOG

  # 4m. Eine Panik ohne Meldung, gefolgt vom Kopf des nächsten Tests, und eine
  # ohne Meldung vor einem Backtrace.
  cat >"$work/heads.log" <<'LOG'
failures:

---- a::first stdout ----
thread 'a::first' panicked at src/one.rs:1:1:
---- a::second stdout ----
thread 'a::second' panicked at src/two.rs:2:2:
stack backtrace:
   0: rust_begin_unwind

failures:

    a::first
    a::second

test result: FAILED. 0 passed; 2 failed; 0 ignored
LOG

  # 4n. Eine sehr lange Meldung: Sie wird gekürzt und sagt das.
  {
    echo "failures:"
    echo ""
    echo "---- a::long stdout ----"
    echo "thread 'a::long' panicked at src/lib.rs:3:3:"
    printf 'the message '
    n=0
    while [ "$n" -lt 40 ]; do
      printf 'is very long indeed '
      n=$((n + 1))
    done
    echo ""
    echo ""
    echo "failures:"
    echo ""
    echo "    a::long"
    echo ""
    echo "test result: FAILED. 0 passed; 1 failed; 0 ignored"
  } >"$work/long.log"

  # 4o. Eine Namensliste, deren Zeile selbst `panicked at` trägt: Sie ist ein
  # Name und kein Grund.
  cat >"$work/listed.log" <<'LOG'
failures:

---- a::listed stdout ----
thread 'a::listed' panicked at src/lib.rs:5:5:
the real reason

failures:

    a::listed

test result: FAILED. 0 passed; 1 failed; 0 ignored
LOG

  # 5. Ein Lauf ohne einen einzigen gefallenen Test.
  cat >"$work/green.log" <<'LOG'
running 2 tests
test a ... ok
test b ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
LOG

  out=$(report "$work/two.log")
  # Der erste Test hat eine Meldung, also steht sie hinter dem Namen: Der Ort
  # und die Zeile der Zusicherung sind das, was ein roter Lauf von außen sonst
  # nicht hergibt.
  expect_line "$out" \
    "^::error::rust-test failed: cmd::run::tests::first -- src/lib.rs:1:1: assertion failed$" \
    "the first name is missing, or without its reason"
  # Der zweite hat keine: Dann steht der Name allein da und nichts Erfundenes.
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
  # Die Panik dieses Falls trägt eine Zeile `failures:` unter sich; sie ist
  # kein Grund, also steht der Ort allein da und nichts dahinter.
  expect_line "$panic" "^::error::rust-test failed: a::real -- src/lib.rs:1:1$" \
    "the real name is missing, or a section header became its reason"
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

  reason=$(report "$work/reason.log")
  expect_line "$reason" \
    "^::error::rust-test failed: a::loud -- crates/ipc/tests/terminal.rs:795:5: the reader gets the scrollback" \
    "the assertion of a failing test is missing"
  if printf '%s\n' "$reason" | grep -q "RUST_BACKTRACE"; then
    echo "self-test: the note of the panic became the reason" >&2
    printf '%s\n' "$reason" >&2
    exit 1
  fi
  if printf '%s\n' "$reason" | grep -q "rust_begin_unwind"; then
    echo "self-test: a backtrace frame became the reason" >&2
    printf '%s\n' "$reason" >&2
    exit 1
  fi
  # Eine Panik ohne eigene Meldung: der Ort steht da, mehr nicht.
  expect_line "$reason" "^::error::rust-test failed: a::quiet -- src/lib.rs:9:9$" \
    "a panic without a message lost its place"

  chatty=$(report "$work/chatty.log")
  expect_line "$chatty" \
    "^::error::rust-test failed: a::chatty -- src/lib.rs:7:7: the door stayed open$" \
    "a test that prints its own dashes lost its reason"

  quoting=$(report "$work/quoting.log")
  expect_line "$quoting" \
    "^::error::rust-test failed: a::quoting -- src/lib.rs:26:9: the door stayed open$" \
    "the test's own output became its reason"

  values=$(report "$work/values.log")
  expect_line "$values" \
    "^::error::rust-test failed: a::values -- src/lib.rs:7:9: assertion .left == right. failed: the reader gets the scrollback left: \[1, 2, 3\] right: \[1, 9, 3\]$" \
    "the values of the assertion are missing"

  twice=$(report "$work/twice.log")
  expect_line "$twice" "^::error::rust-test failed: a::twice -- src/first.rs:1:1$" \
    "the second panic became the reason of the first"

  heads=$(report "$work/heads.log")
  expect_line "$heads" "^::error::rust-test failed: a::first -- src/one.rs:1:1$" \
    "the head of the next test became a reason"
  expect_line "$heads" "^::error::rust-test failed: a::second -- src/two.rs:2:2$" \
    "a backtrace became a reason"

  long=$(report "$work/long.log")
  expect_line "$long" "\.\.\.$" "a very long message was not cut"
  cut_len=$(printf '%s\n' "$long" | head -n 1 | wc -c)
  [ "$cut_len" -lt 360 ] || {
    echo "self-test: the cut message is $cut_len characters long" >&2
    exit 1
  }

  listed=$(report "$work/listed.log")
  expect_line "$listed" "^::error::rust-test failed: a::listed -- src/lib.rs:5:5: the real reason$" \
    "a name in the list became a reason"

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

  echo "test-report: self-test ok (21 cases)"
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
