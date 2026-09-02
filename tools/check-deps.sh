#!/usr/bin/env bash
# Enforce the architecture rules that a compiler cannot: dependency direction,
# crate documentation, and the single egress point. See HUM-074, ADR-015.
set -euo pipefail
cd "$(dirname "$0")/.."

mkdir -p daemon/target
(cd daemon && cargo metadata --format-version 1 --no-deps > target/deps-meta.json)
python3 tools/check_deps.py daemon/target/deps-meta.json tools/deps-allow.toml

fail=0
for lib in daemon/crates/*/src/lib.rs; do
  if ! grep -q '#!\[deny(missing_docs)\]' "$lib"; then
    echo "missing #![deny(missing_docs)]: $lib" >&2
    fail=1
  fi
done

# Every upstream connection goes through the Egress port (ADR-017).
if grep -rn 'TcpStream::connect' daemon/crates daemon/bin --include='*.rs' 2>/dev/null \
    | grep -v 'crates/proxy/src/egress/' | grep -v '/tests/' | grep -v '#\[cfg(test)\]'; then
  echo "TcpStream::connect outside crates/proxy/src/egress/" >&2
  fail=1
fi

# No feature imports another feature; they talk through core only (ARCHITECTURE 5).
if [[ -d app/lib/features ]]; then
  while IFS= read -r hit; do
    src_feature=$(sed -E 's#^app/lib/features/([^/]+)/.*#\1#' <<<"${hit%%:*}")
    dst_feature=$(sed -E "s#.*features/([^/']+)/.*#\1#" <<<"${hit#*:}")
    if [[ -n "$dst_feature" && "$src_feature" != "$dst_feature" ]]; then
      echo "feature imports another feature: $hit" >&2
      fail=1
    fi
  done < <(grep -rn "import .*features/" app/lib/features --include='*.dart' 2>/dev/null || true)
fi

exit "$fail"
