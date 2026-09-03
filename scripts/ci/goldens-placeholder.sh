#!/usr/bin/env sh
# Golden tests, or an honest skip while there are none (HUM-054).
#
# `flutter test test/goldens` on a directory that holds only .gitkeep does not
# reliably exit 0, so the presence of a golden test is decided here instead of
# being left to the tool.
set -eu
cd "$(dirname "$0")/../.."

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

echo "running $count golden test file(s)"
cd app
flutter test --tags golden test/goldens
