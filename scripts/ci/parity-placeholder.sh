#!/usr/bin/env sh
# UI and CLI parity check, or an honest skip while the table does not exist yet
# (HUM-078, ADR-018).
#
# Once xtask can emit it, the check is: regenerate docs/reference/parity.md and
# fail when it moves, because a moved table means an RPC without a CLI
# subcommand.
set -eu
cd "$(dirname "$0")/../.."

if [ ! -f daemon/xtask/src/parity.rs ]; then
  echo "::notice::parity table not yet generated (HUM-078)"
  if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    {
      echo "### parity-check: skipped"
      echo
      echo "\`daemon/xtask/src/parity.rs\` does not exist. The RPC/CLI/UI parity table"
      echo "arrives with HUM-078. No RPC was checked for a CLI counterpart."
    } >> "$GITHUB_STEP_SUMMARY"
  fi
  exit 0
fi

( cd daemon && cargo xtask docs )
git diff --exit-code -- docs/reference/parity.md || {
  echo "::error::docs/reference/parity.md is stale, run cargo xtask docs and commit the result"
  exit 1
}
