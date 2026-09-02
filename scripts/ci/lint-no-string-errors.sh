#!/usr/bin/env sh
# Öffentliche Signaturen in den Bibliotheks-Crates dürfen keine String- oder
# anyhow-Fehler tragen (HUM-063, ADR-012). Ein Fehler ist ein Wert mit
# Bedeutung: `Diagnostic` oder ein thiserror-Typ der Crate.
#
#   scripts/ci/lint-no-string-errors.sh              prüft daemon/crates
#   scripts/ci/lint-no-string-errors.sh DIR          prüft DIR
#   scripts/ci/lint-no-string-errors.sh --self-test  prüft den Prüfer selbst
#
# Der Prüfer selbst ist ein kleines Python-Programm (unten im Heredoc): Es liest
# jede `.rs`-Datei, blendet Kommentare, Literale und `#[cfg(test)]`-Items aus,
# fügt jede `pub fn`-Signatur bis zu ihrem `{` oder `;` über Zeilen hinweg
# zusammen und zerlegt den Rückgabetyp mit ausbalancierten spitzen Klammern.
# Ein zeilenweises grep sah weder `Result<Vec<u8>, String>` noch mehrzeilige
# Signaturen. Verzeichnisse namens `tests/` bleiben außen vor.
#
# Exit 0: nichts gefunden bzw. Selbsttest bestanden. Exit 1: Fundstellen bzw.
# Selbsttest gescheitert.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

if ! command -v python3 >/dev/null 2>&1; then
  echo "lint-no-string-errors: python3 is required" >&2
  exit 1
fi

if [ "${1:-}" = "--self-test" ]; then
  set -- --self-test "$script_dir/fixtures"
elif [ "$#" -eq 0 ]; then
  cd "$script_dir/../.."
  set -- daemon/crates
fi

python3 - "$@" <<'PY'
import os
import re
import shutil
import sys
import tempfile

# `pub fn`, `pub async fn`, `pub const fn`, `pub unsafe fn`. `pub(crate)` ist
# keine öffentliche Schnittstelle und bleibt wie bisher unbeachtet.
PUB_FN = re.compile(r"\bpub\s+(?:(?:async|const|unsafe)\s+)*fn\s+(\w+)")
CFG_TEST = re.compile(r"#\[\s*cfg\s*\(\s*test\s*\)\s*\]")
# `Result<`, auch mit Pfad davor (`std::result::Result<`, `anyhow::Result<`).
RESULT = re.compile(r"(?<!\w)((?:\w+::)*)Result\s*<")
RAW_STRING = re.compile(r'(?:b|c)?r(#*)"')
# Aliasse, die den Fehlertyp verstecken: `anyhow::Result<T>` ist
# `Result<T, anyhow::Error>`.
ALIAS_PREFIXES = ("anyhow::", "eyre::", "color_eyre::")

# Verbotene Fehlertypen, geprüft gegen den normalisierten zweiten Typparameter.
BAD_ERRORS = (
    ("String", re.compile(r"(?:(?:std|alloc)::string::)?String")),
    ("&str", re.compile(r"&(?:'\w+ )?(?:mut )?str")),
    ("anyhow::Error", re.compile(r"anyhow::Error")),
    (
        "Box<dyn Error>",
        re.compile(r"Box<dyn (?:(?:std|core)::error::)?Error(?:\+[\w:']+)*>"),
    ),
    ("eyre::Report", re.compile(r"(?:color_)?eyre::Report")),
)

# Jede `pub fn` der Negativ-Fixture muss der Selbsttest wiederfinden.
EXPECTED_HIT = re.compile(r"^pub\s+(?:(?:async|const|unsafe)\s+)*fn\s+(\w+)")


def blank(chars, start, end):
    """Ersetzt chars[start:end] durch Leerzeichen, Zeilenumbrüche bleiben."""
    for k in range(start, end):
        if chars[k] != "\n":
            chars[k] = " "


def mask(text):
    """Kommentare und String-/Zeichenliterale werden zu Leerzeichen.

    Länge und Zeilenumbrüche bleiben erhalten, Offsets und Zeilennummern
    gelten also weiterhin für den Originaltext.
    """
    out = list(text)
    n = len(text)
    i = 0
    while i < n:
        c = text[i]
        word_before = i > 0 and (text[i - 1].isalnum() or text[i - 1] == "_")
        if text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j < 0 else j
            blank(out, i, j)
            i = j
        elif text.startswith("/*", i):
            depth = 1
            j = i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth += 1
                    j += 2
                elif text.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            blank(out, i, j)
            i = j
        elif c in "bcr" and not word_before and RAW_STRING.match(text, i):
            m = RAW_STRING.match(text, i)
            closing = '"' + "#" * len(m.group(1))
            j = text.find(closing, m.end())
            j = n if j < 0 else j + len(closing)
            blank(out, i, j)
            i = j
        elif c == '"' or (c in "bc" and not word_before and text.startswith('"', i + 1)):
            j = i + 1 if c == '"' else i + 2
            while j < n:
                if text[j] == "\\":
                    j += 2
                elif text[j] == '"':
                    j += 1
                    break
                else:
                    j += 1
            blank(out, i, min(j, n))
            i = min(j, n)
        elif c == "'":
            if text.startswith("\\", i + 1):
                j = text.find("'", i + 3)
                j = i + 3 if j < 0 else j + 1
                blank(out, i, min(j, n))
                i = min(j, n)
            elif i + 2 < n and text[i + 2] == "'":
                blank(out, i, i + 3)
                i += 3
            else:
                i += 1  # Lifetime
        else:
            i += 1
    return "".join(out)


def skip_attribute(text, i):
    """i zeigt auf das `[` eines Attributs; liefert den Index hinter dem `]`."""
    depth = 0
    n = len(text)
    while i < n:
        if text[i] == "[":
            depth += 1
        elif text[i] == "]":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return n


def item_end(text, i):
    """Ende des Items ab i: `;` auf Tiefe 0 oder die schließende `}`."""
    depth = 0
    n = len(text)
    while i < n:
        c = text[i]
        if c in "([{":
            depth += 1
        elif c in ")]}":
            if depth == 0:
                return i  # schließt einen umgebenden Block
            depth -= 1
            if depth == 0 and c == "}":
                return i + 1
        elif c == ";" and depth == 0:
            return i + 1
        i += 1
    return n


def exempt_cfg_test(text):
    """Blendet jedes Item hinter `#[cfg(test)]` aus (Modul oder Funktion)."""
    out = list(text)
    n = len(text)
    for m in CFG_TEST.finditer(text):
        i = m.end()
        while True:
            while i < n and text[i].isspace():
                i += 1
            if text.startswith("#[", i):
                i = skip_attribute(text, i + 1)
            else:
                break
        blank(out, m.start(), item_end(text, i))
    return "".join(out)


def signature_end(text, i):
    """Erstes `{` oder `;` auf Klammertiefe 0 ab i."""
    depth = 0
    n = len(text)
    while i < n:
        if text.startswith("->", i):
            i += 2
            continue
        c = text[i]
        if c in "([<":
            depth += 1
        elif c in ")]>":
            depth -= 1
        elif depth == 0 and c in "{;":
            return i
        i += 1
    return n


def return_type(sig):
    """Der Typ hinter dem `->` auf Tiefe 0, ohne `where`-Klausel."""
    depth = 0
    i = 0
    n = len(sig)
    while i < n:
        if sig.startswith("->", i):
            if depth == 0:
                return strip_where(sig[i + 2 :])
            i += 2
            continue
        c = sig[i]
        if c in "([<":
            depth += 1
        elif c in ")]>":
            depth -= 1
        i += 1
    return ""


def strip_where(text):
    depth = 0
    i = 0
    n = len(text)
    while i < n:
        if text.startswith("->", i):
            i += 2
            continue
        c = text[i]
        if c in "([<":
            depth += 1
        elif c in ")]>":
            depth -= 1
        elif (
            depth == 0
            and text.startswith("where", i)
            and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_"))
            and not (i + 5 < n and (text[i + 5].isalnum() or text[i + 5] == "_"))
        ):
            return text[:i]
        i += 1
    return text


def generic_args(text, i):
    """Typargumente der spitzen Klammer, die vor i geöffnet wurde."""
    args = []
    angle = 0
    other = 0
    start = i
    n = len(text)
    while i < n:
        if text.startswith("->", i):
            i += 2
            continue
        c = text[i]
        if c == "<":
            angle += 1
        elif c == ">":
            if angle == 0:
                args.append(text[start:i].strip())
                return args
            angle -= 1
        elif c in "([":
            other += 1
        elif c in ")]":
            other -= 1
        elif c == "," and angle == 0 and other == 0:
            args.append(text[start:i].strip())
            start = i + 1
        i += 1
    args.append(text[start:].strip())
    return args


def normalize(type_text):
    s = re.sub(r"\s+", " ", type_text).strip()
    s = re.sub(r"\s*([<>,+&])\s*", r"\1", s)
    return re.sub(r"\s*::\s*", "::", s)


def bad_error_types(ret):
    """Namen der verbotenen Fehlertypen in einem Rückgabetyp, beliebig tief."""
    found = []
    for m in RESULT.finditer(ret):
        args = generic_args(ret, m.end())
        prefix = m.group(1)
        if prefix in ALIAS_PREFIXES and len(args) == 1:
            found.append(prefix + "Result")
        if len(args) >= 2:
            err = normalize(args[1])
            for name, pattern in BAD_ERRORS:
                if pattern.fullmatch(err):
                    found.append(name)
    return found


def tidy(signature):
    s = re.sub(r"\s+", " ", signature).strip()
    s = re.sub(r"\(\s+", "(", s)
    return re.sub(r",?\s+\)", ")", s)


class Hit:
    def __init__(self, path, line, name, signature, errors):
        self.path = path
        self.line = line
        self.name = name
        self.signature = signature
        self.errors = errors


def scan_text(path, text):
    masked = exempt_cfg_test(mask(text))
    for m in PUB_FN.finditer(masked):
        end = signature_end(masked, m.end())
        errors = bad_error_types(return_type(masked[m.end() : end]))
        if errors:
            line = masked.count("\n", 0, m.start()) + 1
            yield Hit(path, line, m.group(1), tidy(masked[m.start() : end]), errors)


def rust_files(root):
    if os.path.isfile(root):
        yield root
        return
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d != "tests")
        for name in sorted(filenames):
            if name.endswith(".rs"):
                yield os.path.join(dirpath, name)


def scan(roots):
    for root in roots:
        for path in rust_files(root):
            with open(path, encoding="utf-8", errors="replace") as fh:
                yield from scan_text(path, fh.read())


def self_test(fixtures):
    tmp = tempfile.mkdtemp(prefix="lint-no-string-errors.")
    try:
        bad = os.path.join(tmp, "bad_signature.rs")
        good = os.path.join(tmp, "good_signature.rs")
        exempt = os.path.join(tmp, "tests", "exempt.rs")
        os.mkdir(os.path.dirname(exempt))
        shutil.copy(os.path.join(fixtures, "bad_signature.rs.txt"), bad)
        shutil.copy(os.path.join(fixtures, "good_signature.rs.txt"), good)
        shutil.copy(os.path.join(fixtures, "bad_signature.rs.txt"), exempt)

        with open(bad, encoding="utf-8") as fh:
            lines = fh.read().splitlines()
        expected = set()
        for number, line in enumerate(lines, 1):
            m = EXPECTED_HIT.match(line)
            if m:
                expected.add((number, m.group(1)))

        hits = list(scan([tmp]))
        got = {(h.line, h.name) for h in hits if h.path == bad}
        problems = []
        if not expected:
            problems.append("the bad fixture declares no public fn")
        for line, name in sorted(expected - got):
            problems.append(f"missed: bad_signature.rs:{line}: {name}")
        for line, name in sorted(got - expected):
            problems.append(f"unexpected: bad_signature.rs:{line}: {name}")
        for h in hits:
            if h.path != bad:
                rel = os.path.relpath(h.path, tmp)
                problems.append(f"flagged although good or exempt: {rel}:{h.line}: {h.signature}")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    if problems:
        print("self-test failed:", file=sys.stderr)
        for p in problems:
            print("  " + p, file=sys.stderr)
        return 1
    print(f"self-test ok ({len(expected)} bad signatures detected, none stray)")
    return 0


def main(argv):
    if argv and argv[0] == "--self-test":
        return self_test(argv[1])
    hits = list(scan(argv))
    for h in hits:
        print(f"{h.path}:{h.line}: {h.signature}")
    if hits:
        print("::error::public fns must return typed errors (Diagnostic or a thiserror type), see HUM-063")
        return 1
    return 0


sys.exit(main(sys.argv[1:]))
PY
