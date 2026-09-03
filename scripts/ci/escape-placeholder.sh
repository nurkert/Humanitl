#!/usr/bin/env sh
# Placeholder for the escape tests (HUM-006).
#
# ci.yml calls this only while tests/escape/run.sh does not exist. The harness
# is the real thing; this stands in for nothing but its absence. It writes an
# empty JUnit file so the artefact step has something to hand over, prints a
# notice and puts a skip block into the job summary. It never reports a pass.
set -eu
cd "$(dirname "$0")/../.."

mkdir -p target/escape
cat > target/escape/placeholder.xml <<'XML'
<testsuite name="escape" tests="0" skipped="0"><!-- HUM-006 not yet implemented --></testsuite>
XML

echo "::notice::escape tests not yet implemented (HUM-006)"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  {
    echo "### escape-tests: skipped"
    echo
    echo "\`tests/escape/run.sh\` does not exist. The escape harness arrives with HUM-006."
    echo "No probe was run."
  } >> "$GITHUB_STEP_SUMMARY"
fi
