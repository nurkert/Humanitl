#!/usr/bin/env sh
# Placeholder for the end-to-end walkthrough (HUM-021).
#
# One user path per milestone runs here under xvfb. Until the first one exists
# the job reports a skip; HUM-021 replaces the call site with
# `xvfb-run -a ./tests/e2e/run.sh`.
set -eu
cd "$(dirname "$0")/../.."

mkdir -p target/e2e
echo "e2e not yet implemented (HUM-021)" > target/e2e/placeholder.txt

echo "::notice::end-to-end tests not yet implemented (HUM-021)"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  {
    echo "### e2e-xvfb: skipped"
    echo
    echo "The end-to-end walkthrough arrives with HUM-021. No user path was exercised."
  } >> "$GITHUB_STEP_SUMMARY"
fi
