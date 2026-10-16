#!/usr/bin/env sh
# Golden tests, or an honest skip while there are none (HUM-054).
#
# `flutter test test/goldens` on a directory that holds only .gitkeep does not
# reliably exit 0, so the presence of a golden test is decided here instead of
# being left to the tool.
set -eu
cd "$(dirname "$0")/../.."

# Der Job flutter-analyze-test laesst die Goldens seit HUM-237 aus
# (`--exclude-tags golden`), dieser Job faehrt nur `app/test/goldens`. Ein
# Golden-Test anderswo liefe also in keinem der beiden; er macht den Job rot,
# statt still herauszufallen.
stray=$(grep -rlE 'goldenTest\(|matchesGoldenFile\(' app/test app/packages/ui/test 2>/dev/null \
  | grep -v '^app/test/goldens/' || true)
if [ -n "$stray" ]; then
  echo "::error::golden tests outside app/test/goldens run in no CI job; move them there:"
  echo "$stray"
  exit 1
fi

count=$(find app/test/goldens -type f -name '*_test.dart' 2>/dev/null | wc -l | tr -d ' ')

if [ "$count" -eq 0 ]; then
  echo "::notice::no golden tests yet (HUM-054)"
  if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    {
      echo "### goldens: skipped"
      echo
      echo "\`app/test/goldens\` holds no \`*_test.dart\`. Golden coverage arrives with HUM-054."
      echo "No pixel was compared."
    } >> "$GITHUB_STEP_SUMMARY"
  fi
  exit 0
fi

# Ein Testprozess je Kern (HUM-237). Die Pixel haengen daran nicht: Jede Datei
# rendert in ihrem eigenen Prozess, mit den CI-Schriften von alchemist
# (`app/test/flutter_test_config.dart`) und ohne geteilten Zustand.
jobs=$(nproc 2>/dev/null || echo 1)
echo "running $count golden test file(s), $jobs at a time"
cd app
flutter test --tags golden --concurrency="$jobs" test/goldens
