#!/usr/bin/env bash
# Test fuer tools/ci-changes.sh (HUM-424) in einem Wegwerf-Repository.
#
# Jeder Fall legt auf einer Basis einen Commit an und prueft, welche Jobs von
# ci.yml das Skript dafuer verlangt. Geprueft wird vor allem, was nie
# geschehen darf: ein uebersprungener Job, dessen Eingaben sich geaendert
# haben, und eine Pipeline-Aenderung, die nicht alles startet.
#
# Mutationsbeweis: CI_CHANGES=<pfad> prueft eine veraenderte Kopie des Skripts.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
script="${CI_CHANGES:-$here/../ci-changes.sh}"

work="$(mktemp -d "${TMPDIR:-/tmp}/ci-changes-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT
fails=0

export GIT_AUTHOR_NAME=Test GIT_AUTHOR_EMAIL=test@example.invalid
export GIT_COMMITTER_NAME=Test GIT_COMMITTER_EMAIL=test@example.invalid
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1

repo="$work/repo"
git init -q -b main "$repo"
g() { git -C "$repo" "$@"; }
put() { # <pfad> <inhalt>
  mkdir -p "$(dirname "$repo/$1")"
  printf '%s\n' "$2" > "$repo/$1"
}
put daemon/crates/proxy/src/lib.rs 'pub fn plain() {}'
put daemon/crates/sandbox/src/lib.rs '#[cfg(target_os = "macos")] mod seat;'
put daemon/Cargo.lock 'version = 4'
put app/lib/main.dart 'void main() {}'
put app/lib/platform.dart 'bool get mac => Platform.isMacOS;'
put app/l10n/app_en.arb '{"hint": "Open it on macOS"}'
put docs/CONFIG.md '# Config'
put backlog/sprint-1.md '# Sprint'
put website/index.md '# Site'
put tools/x.sh 'echo'
put proto/humanitl.proto 'syntax = "proto3";'
put README.md '# Humanitl'
g add -A
g commit -q -m base
base="$(g rev-parse HEAD)"

# change <name> <pfad> <inhalt> [<pfad> <inhalt> ...]: ein Commit auf base,
# als Zweig <name>. Inhalt `-` loescht die Datei.
change() {
  local name="$1"
  shift
  g checkout -q -B "$name" "$base"
  while [[ $# -gt 0 ]]; do
    if [[ "$2" == - ]]; then
      g rm -q "$1"
    else
      put "$1" "$2"
      g add "$1"
    fi
    shift 2
  done
  g commit -q -m "$name"
}

# expect <fall> <argumente fuer das skript> -- <schluessel=wert> ...
expect() {
  local name="$1" args=() out rc=0 want bad=""
  shift
  while [[ "$1" != -- ]]; do
    args+=("$1")
    shift
  done
  shift
  out="$(cd "$repo" && "$script" "${args[@]}" 2> "$work/err")" || rc=$?
  if ((rc != 0)); then
    echo "FAIL $name: exit $rc"
    sed 's/^/    /' "$work/err"
    fails=$((fails + 1))
    return
  fi
  for want in "$@"; do
    grep -qx -- "$want" <<< "$out" || bad+=" $want"
  done
  if [[ -n "$bad" ]]; then
    echo "FAIL $name: erwartet$bad"
    sed 's/^/    /' <<< "$out"
    fails=$((fails + 1))
  else
    echo "ok   $name"
  fi
}

heavy=(rust-check rust-test macos deps-lint proto-lint-and-gen flutter-analyze-test goldens
  escape-tests escape-tests-podman kvm-tests e2e e2e-xvfb e2e-agent e2e-m4 e2e-m4-package
  parity-check)
all_false=("${heavy[@]/%/=false}")
all_true=("${heavy[@]/%/=true}")

# Nur website/: kein schwerer Job, auch auf main nicht.
change website website/index.md '# New site'
expect website_branch --base "$base" website -- full=false "${all_false[@]}"
expect website_main --main --base "$base" website -- full=false "${all_false[@]}"
# backlog/ und BACKLOG.md: nur rust-test, das config_readers.rs sie liest.
change backlog backlog/sprint-1.md '# Sprint 1, done' website/index.md '# x'
expect backlog_main --main --base "$base" backlog -- full=false area-backlog=true rust-test=true \
  rust-check=false macos=false deps-lint=false proto-lint-and-gen=false flutter-analyze-test=false \
  goldens=false escape-tests=false escape-tests-podman=false kvm-tests=false e2e=false \
  e2e-xvfb=false e2e-agent=false e2e-m4=false e2e-m4-package=false parity-check=false
change backlog_root BACKLOG.md '# Plan'
expect backlog_root_file --base "$base" backlog_root -- area-backlog=true rust-test=true goldens=false
# CHANGELOG.md allein ist der Commit eines Releases: alles (HUM-424).
change changelog CHANGELOG.md '# Changelog'
expect changelog_full --main --base "$base" changelog -- full=true "${all_true[@]}"

# docs/: nur die Linux-Jobs, die docs/ lesen; kein macOS, auch auf main nicht.
change docs docs/CONFIG.md '# Config, new' README.md '# Humanitl, new'
expect docs_main --main --base "$base" docs -- full=false area-docs=true \
  rust-test=true flutter-analyze-test=true deps-lint=true parity-check=true \
  rust-check=false macos=false goldens=false proto-lint-and-gen=false escape-tests=false \
  escape-tests-podman=false kvm-tests=false e2e=false e2e-xvfb=false e2e-agent=false e2e-m4=false \
  e2e-m4-package=false

# App ohne macOS-Bezug: auf dem Zweig kein macOS, auf main schon. Ein ARB-Text,
# der macOS nennt, ist kein macOS-Code.
change app app/lib/main.dart 'void main() { run(); }' app/l10n/app_en.arb '{"hint": "Open it on macOS now"}'
expect app_branch --base "$base" app -- area-app=true area-rust=false macos=false goldens=true \
  flutter-analyze-test=true e2e-xvfb=true e2e-m4=true rust-check=false e2e=false escape-tests=false \
  escape-tests-podman=false
expect app_main --main --base "$base" app -- macos=true goldens=true rust-check=false

# macOS-naher Code: Pfad, Inhalt (alt oder neu), Manifest.
change app_mac app/lib/platform.dart 'bool get mac => Platform.isMacOS && true;'
expect app_mac_content --base "$base" app_mac -- area-macos=true macos=true
change app_runner app/macos/Runner/MainFlutterWindow.swift 'class W {}'
expect app_mac_path --base "$base" app_runner -- macos=true
change rust_plain daemon/crates/proxy/src/lib.rs 'pub fn plain() { }'
expect rust_plain_branch --base "$base" rust_plain -- area-rust=true macos=false rust-check=true \
  rust-test=true escape-tests=true escape-tests-podman=true kvm-tests=true e2e=true e2e-agent=true \
  goldens=false
expect rust_plain_main --main --base "$base" rust_plain -- macos=true
change rust_mac daemon/crates/sandbox/src/lib.rs 'mod seat;'
expect rust_mac_old_content --base "$base" rust_mac -- macos=true
change rust_lock daemon/Cargo.lock 'version = 4 # bumped'
expect rust_lockfile --base "$base" rust_lock -- macos=true
change rust_gone daemon/crates/sandbox/src/lib.rs -
expect rust_deleted_mac_file --base "$base" rust_gone -- macos=true

# proto/ betrifft Daemon und App.
change proto proto/humanitl.proto 'syntax = "proto3"; // x'
expect proto --base "$base" proto -- area-rust=true area-app=true rust-check=true goldens=true macos=false

# Pipeline und Unbekanntes: alles.
change tools tools/x.sh 'echo 2'
expect tools_full --base "$base" tools -- full=true "${all_true[@]}"
change dotfile .editorconfig 'root = true'
expect unknown_root_file_full --base "$base" dotfile -- full=true "${all_true[@]}"
change unknown_dir newdir/notes.md 'x'
expect unknown_dir_md_full --base "$base" unknown_dir -- full=true
change app_md app/README.md '# App'
expect app_md_is_app --base "$base" app_md -- full=false area-app=true area-docs=false

# Keine brauchbare Basis: alles.
expect full_flag --full website -- full=true "${all_true[@]}"
expect zero_before --main --base 0000000000000000000000000000000000000000 website -- full=true macos=true
expect empty_before --main --base "" website -- full=true
expect unknown_before --main --base 1234567 website -- full=true
expect forced_push --main --base docs website -- full=true

# git scheitert: nie ein stilles Auslassen, sondern alles. Ein Commit-Objekt,
# das im Klon fehlt (flacher Klon ohne die Basis), sieht rev-parse noch als
# Namen nicht; deshalb hier ein Baum, den git kennt, als Basis ohne Commit,
# und ein beschaedigtes Objekt fuer den diff selbst.
expect unreachable_base_sha --main --base 0123456789abcdef0123456789abcdef01234567 website -- full=true "${all_true[@]}"
expect tree_as_base--main --base "$(g rev-parse "$base^{tree}")" website -- full=true "${all_true[@]}"
broken="$work/broken"
# Eine Kopie mit losen Objekten; ein Klon packte sie.
cp -a "$repo" "$broken"
# Den Baum von docs/ im Basis-Commit entfernen: rev-parse und merge-base
# gelingen, git diff scheitert beim Lesen.
docs_tree="$(git -C "$broken" rev-parse "$base:docs")"
obj="$broken/.git/objects/${docs_tree:0:2}/${docs_tree:2}"
if [[ -f "$obj" ]]; then
  rm -f "$obj"
  out="$(cd "$broken" && "$script" --main --base "$base" docs 2> "$work/err-broken")" || true
  if grep -qx full=true <<< "$out" && grep -qx rust-check=true <<< "$out" && grep -q '::warning' "$work/err-broken"; then
    echo "ok   unreadable_base_diff_runs_everything"
  else
    echo "FAIL unreadable_base_diff_runs_everything"
    sed 's/^/    /' <<< "$out"
    sed 's/^/    /' "$work/err-broken"
    fails=$((fails + 1))
  fi
  # Eine Datei mit macOS-Merkmal, deren alter Blob fehlt: alles statt macos=false.
  mac_blob="$(git -C "$broken" rev-parse "$base:daemon/crates/sandbox/src/lib.rs")"
  rm -f "$broken/.git/objects/${mac_blob:0:2}/${mac_blob:2}"
  out="$(cd "$broken" && "$script" --base "$base" rust_mac 2> "$work/err-blob")" || true
  if grep -qx full=true <<< "$out" && grep -qx macos=true <<< "$out"; then
    echo "ok   unreadable_blob_runs_everything"
  else
    echo "FAIL unreadable_blob_runs_everything"
    sed 's/^/    /' <<< "$out"
    sed 's/^/    /' "$work/err-blob"
    fails=$((fails + 1))
  fi
else
  echo "FAIL unreadable_base_diff_runs_everything: object $docs_tree is packed, test cannot break it"
  fails=$((fails + 1))
fi

# Zweige vergleichen mit der Basis zu main: Was main seither bekam, zaehlt nicht.
g checkout -q -B main "$base"
put daemon/crates/proxy/src/lib.rs 'pub fn moved_on() {}'
g add -A
g commit -q -m "main moves"
expect branch_merge_base --merge-base-with main website -- full=false "${all_false[@]}"
expect no_merge_base --merge-base-with no-such-ref website -- full=true

# Falscher Aufruf.
if (cd "$repo" && "$script" website > /dev/null 2>&1); then
  echo "FAIL usage_without_mode"
  fails=$((fails + 1))
else
  echo "ok   usage_without_mode"
fi

if [[ "$fails" -ne 0 ]]; then
  echo "ci_changes_test: $fails Fall/Faelle rot" >&2
  exit 1
fi
echo "ci_changes_test: alle Faelle gruen"
