#!/usr/bin/env bash
# Commit one issue on its own branch and merge it into main, as CONTRIBUTING
# requires. Usage: tools/commit-issue.sh HUM-003 proto-v1 "subject line" <<'MSG'
# body ...
# MSG
set -euo pipefail
cd "$(dirname "$0")/.."

issue="${1:?issue id, e.g. HUM-003}"
slug="${2:?branch slug, e.g. proto-v1}"
subject="${3:?commit subject}"
shift 3
body="$(cat)"

branch="$(tr '[:upper:]' '[:lower:]' <<<"$issue")-$slug"
paths=("$@")
if [[ ${#paths[@]} -eq 0 ]]; then
  echo "no paths given for $issue" >&2
  exit 1
fi

git checkout -q -b "$branch"
git add -- "${paths[@]}"
if git diff --cached --quiet; then
  echo "nothing staged for $issue" >&2
  git checkout -q main
  git branch -q -D "$branch"
  exit 1
fi
git commit -q -m "$subject" -m "$body" -m "Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
git checkout -q main
git merge -q --no-ff "$branch" -m "merge: $issue $slug"
git branch -q -d "$branch"
git --no-pager log --oneline -1
