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
# The shim is exempt: it is the loopback bridge inside the sandbox, has no
# access to the network namespace of the host and never talks to an upstream.
if grep -rn 'TcpStream::connect' daemon/crates daemon/bin --include='*.rs' 2>/dev/null \
    | grep -v 'crates/proxy/src/egress/' | grep -v 'bin/humanitl-shim/' \
    | grep -v '/tests/' | grep -v '#\[cfg(test)\]'; then
  echo "TcpStream::connect outside crates/proxy/src/egress/" >&2
  fail=1
fi

# Ein Skript mit Shebang, das jemand direkt aufruft, braucht das Ausfuehrungsbit;
# die CI ruft ./tests/e2e/run.sh und ./tests/escape/run.sh so auf. Lokal faellt
# das nie auf, weil man `bash skript` tippt; auf dem Runner ist es Exit 126.
while IFS= read -r script; do
  if head -c2 "$script" | grep -q '^#!' && [[ ! -x "$script" ]]; then
    echo "script has a shebang but no executable bit: $script" >&2
    fail=1
  fi
done < <(git ls-files 'tests/*.sh' 'tests/*.py' 'scripts/*.sh' 'tools/*.sh' 2>/dev/null)

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
  # Auch der relative Weg zaehlt: aus `features/a/x/y.dart` fuehrt
  # `../../b/...` in ein anderes Feature, ohne dass "features/" im Text steht.
  while IFS= read -r hit; do
    file="${hit%%:*}"
    src_feature=$(sed -E 's#^app/lib/features/([^/]+)/.*#\1#' <<<"$file")
    # Die Shell ist der Rahmen, der die Features einhaengt (ARCHITECTURE 5):
    # sie darf deren Einstiegs-Screens importieren, sonst niemand.
    [[ "$src_feature" == "shell" ]] && continue
    rel=$(sed -E "s#.*import '([^']+)'.*#\1#" <<<"${hit#*:}")
    target=$(cd "$(dirname "$file")" && realpath -m --relative-to=. "$rel" 2>/dev/null || true)
    target=$(realpath -m --relative-to="$PWD" "$(dirname "$file")/$rel" 2>/dev/null || true)
    case "$target" in
      app/lib/features/*)
        dst_feature=$(sed -E 's#^app/lib/features/([^/]+)/.*#\1#' <<<"$target")
        if [[ -n "$dst_feature" && "$src_feature" != "$dst_feature" ]]; then
          echo "feature imports another feature: $hit" >&2
          fail=1
        fi
        ;;
    esac
  done < <(grep -rn "^import '\.\./" app/lib/features --include='*.dart' 2>/dev/null || true)
fi

# Die Komponentenbibliothek steht hinter der Naht (ADR-0009, revidiert am
# 2026-09-04). Nur `app/packages/ui` darf sie importieren; ein Import aus einem
# Feature, aus `app/lib/core` oder aus einem Test macht die Naht wertlos, denn
# dann haengt ein Bildschirm direkt an einer fremden Bibliothek.
if [[ -d app/lib ]]; then
  while IFS= read -r hit; do
    echo "imports the component library outside packages/ui: $hit" >&2
    fail=1
  done < <(grep -rn "package:shadcn_flutter" app/lib app/test app/integration_test --include='*.dart' 2>/dev/null || true)
fi

# Und die Naht ist auch in der oeffentlichen Schnittstelle des Pakets zu:
# `shadcn_theme.dart` und `h_control.dart` fuehren Typen der Bibliothek in
# ihren Signaturen. Wuerde `humanitl_ui.dart` sie exportieren, koennte ein
# Feature sie benutzen, ohne je `package:shadcn_flutter` zu schreiben — und
# genau nach diesem Import sucht die Pruefung darueber.
barrel=app/packages/ui/lib/humanitl_ui.dart
if [[ -f "$barrel" ]]; then
  while IFS= read -r hit; do
    echo "the barrel exports a file that carries library types: $hit" >&2
    fail=1
  done < <(grep -n "^export 'src/theme/shadcn_theme.dart'\|^export 'src/widgets/h_control.dart'" "$barrel" || true)
fi

# Und sie steht in `app/packages/ui/pubspec.yaml`, nicht im Wurzel-Pubspec:
# ein Eintrag dort machte sie fuer jedes Feature aufloesbar.
if [[ -f app/pubspec.yaml ]] && grep -q '^\s*shadcn_flutter:' app/pubspec.yaml; then
  echo "shadcn_flutter belongs in app/packages/ui/pubspec.yaml, not app/pubspec.yaml" >&2
  fail=1
fi

exit "$fail"
