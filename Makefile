# Humanitl — developer entry points.
# Everything CI runs is reachable from here. `make check` is the gate.

SHELL := /bin/bash
.DEFAULT_GOAL := help

.PHONY: help check rust-fmt rust-clippy rust-build rust-test rust-doc rust-deny typed-errors-lint \
        flutter-get flutter-analyze flutter-test flutter-test-dbus flutter-test-daemon \
        flutter-test-integration flutter-build proto escape e2e \
        deps-lint docs-lint clean

help: ## List targets
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) | sort | awk -F':.*?## ' '{printf "  %-18s %s\n", $$1, $$2}'

check: rust-fmt rust-clippy rust-build rust-test rust-doc deps-lint docs-lint typed-errors-lint flutter-analyze flutter-test flutter-build ## Full local gate (same steps as CI)

# A rustup toolchain may exist without rustup on PATH (this machine): put its
# bin directory first so `cargo fmt` and `cargo clippy` find their components.
RUSTUP_BIN := $(firstword $(wildcard $(HOME)/.rustup/toolchains/*/bin))
# `protoc-gen-dart` liegt nach `dart pub global activate` unter `~/.pub-cache/bin`
# und steht dort in keiner Standard-PATH. Ohne diesen Eintrag überspringt
# `scripts/gen-proto.sh` die Dart-Hälfte still, und der erzeugte Code bleibt auf
# dem Stand von gestern (Befund des Reviews zu HUM-091).
PUB_CACHE_BIN := $(HOME)/.pub-cache/bin
TOOLS_PATH := $(if $(RUSTUP_BIN),$(RUSTUP_BIN):)$(PUB_CACHE_BIN):$(PATH)

rust-fmt: ## cargo fmt --check (skipped when rustfmt is absent)
	@export PATH="$(TOOLS_PATH)"; if cd daemon && cargo fmt --version >/dev/null 2>&1; then cargo fmt --all -- --check; \
	elif [[ -n "$$STRICT" ]]; then echo "rustfmt missing and STRICT set" >&2; exit 1; \
	else echo "SKIP rust-fmt: rustfmt component not installed (rustup component add rustfmt)"; fi

rust-clippy: ## cargo clippy -D warnings (skipped when clippy is absent)
	@export PATH="$(TOOLS_PATH)"; if cd daemon && cargo clippy --version >/dev/null 2>&1; then cargo clippy --workspace --all-targets -- -D warnings; \
	elif [[ -n "$$STRICT" ]]; then echo "clippy missing and STRICT set" >&2; exit 1; \
	else echo "SKIP rust-clippy: clippy component not installed (rustup component add clippy)"; fi

rust-build: ## Build the whole daemon workspace
	cd daemon && cargo build --workspace --all-targets

rust-test: ## Run all Rust tests
	cd daemon && cargo test --workspace

rust-doc: ## Documentation builds without warnings (CI parity)
	cd daemon && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

rust-deny: ## License and advisory audit (needs cargo-deny)
	cd daemon && cargo deny check

typed-errors-lint: ## Typed errors (HUM-063) and the test reporter (HUM-133) check themselves
	scripts/ci/lint-no-string-errors.sh --self-test
	scripts/ci/lint-no-string-errors.sh
	scripts/ci/test-report.sh --self-test

deps-lint: ## Enforce the dependency direction (HUM-074) and the coupling ratchet (HUM-145)
	./tools/check-deps.sh
	python3 tools/tests/check_deps_test.py
	python3 tools/tests/check_offline_test.py
	python3 tools/check_coupling.py
	python3 tools/tests/check_coupling_test.py

docs-lint: ## Check the security documents (HUM-007)
	./scripts/ci/lint-docs.sh

flutter-get: ## Fetch Dart packages (app and packages/ui)
	cd app && flutter pub get
	cd app/packages/ui && flutter pub get

# Der erzeugte Code ist keine Quelle (ARCHITECTURE 4) und steht deshalb nicht im
# Repository. Analyse, Test und Bau haengen daran: In einem frischen Auscheckstand
# fehlen sonst alle .g.dart- und .freezed.dart-Dateien, und `flutter analyze`
# meldet Hunderte Fehler, die keine sind.
flutter-codegen: flutter-get proto ## Generated Dart code: riverpod, freezed, ARB
	@if grep -qE '^[[:space:]]*build_runner[[:space:]]*:' app/pubspec.yaml; then \
	  cd app && dart run build_runner build --delete-conflicting-outputs; \
	else \
	  echo "no build_runner dependency yet, nothing to generate"; \
	fi

flutter-analyze: flutter-codegen ## Static analysis of the Flutter app and packages/ui
	cd app && flutter analyze
	cd app/packages/ui && flutter analyze
	cd app && dart format --output=none --set-exit-if-changed lib test packages/ui/lib packages/ui/test

flutter-build: flutter-codegen ## Debug build of the Linux desktop app (CI parity)
	cd app && flutter build linux --debug

flutter-test: flutter-codegen ## Flutter unit and widget tests (app and packages/ui)
	cd app && flutter test
	cd app/packages/ui && flutter test

# Nicht Teil von `make check`: der Test braucht einen Session-Bus, den CI nicht
# hat. `dbus-run-session` stellt einen eigenen, leeren Bus bereit, und nur dort
# sind `org.kde.StatusNotifierWatcher` und `org.freedesktop.Notifications` frei,
# so dass die Attrappen des Tests sie selbst halten und messen koennen, was auf
# der Leitung steht. Auf dem Bus eines echten Desktops halten Panel und
# Meldungsdienst diese Namen; dort ueberspringt sich der Test mit Grund, statt
# sich beim Waechter des Menschen einzutragen (HUM-118).
flutter-test-dbus: flutter-codegen ## D-Bus protocol tests on a private session bus (HUM-118)
	@command -v dbus-run-session >/dev/null 2>&1 || { echo "dbus-run-session missing: install the dbus package" >&2; exit 1; }
	cd app && dbus-run-session -- env HUMANITL_DBUS_TESTS=1 flutter test test/features/tray/dbus_live_test.dart

# Der Sandbox-Bildschirm gegen einen echten Daemon. Nicht Teil von `check`:
# Der Lauf startet `humanitld` und darunter eine echte Sandbox mit `bwrap`, und
# beides gehoert nicht in ein Gate, das auf jedem Rechner in Sekunden gruen
# sein soll. Er ist die Messung fuer das Kriterium von HUM-040, das frueher
# "manuell mit echtem Daemon" hiess.
flutter-test-daemon: flutter-codegen ## Sandbox screen against a real daemon (HUM-040)
	@test -x daemon/target/debug/humanitld || { echo "daemon/target/debug/humanitld missing: cargo build --manifest-path daemon/Cargo.toml" >&2; exit 1; }
	@command -v bwrap >/dev/null 2>&1 || { echo "bwrap missing: install the bubblewrap package" >&2; exit 1; }
	cd app && env HUMANITL_DAEMON_TESTS=1 flutter test test/features/sandbox/daemon_live_test.dart

# Die Integrationstests: die echte Anwendung auf einem Bildschirm, gegen den
# echten Daemon. Nicht Teil von `check`, aus denselben zwei Gruenden wie die
# beiden Ziele darueber: Sie brauchen einen Bildschirm (`Xvfb :99` genuegt) und
# starten Prozesse. Wer sie faehrt, prueft damit die Naht, die kein
# Widget-Test sieht -- was die Oberflaeche zeigt, wenn ein echter Dienst
# antwortet (HUM-029, HUM-097).
flutter-test-integration: flutter-codegen ## The app on a screen, against a real daemon (HUM-097)
	@test -x daemon/target/debug/humanitld || { echo "daemon/target/debug/humanitld missing: cargo build --manifest-path daemon/Cargo.toml" >&2; exit 1; }
	@test -n "$$DISPLAY" || { echo "no DISPLAY: start one with 'Xvfb :99 -screen 0 1600x1000x24 &' and export DISPLAY=:99" >&2; exit 1; }
	@# Eine Datei nach der anderen: Mehrere Dateien in einem Aufruf starten die
	@# Anwendung mehrfach auf demselben Geraet, und der zweite Start scheitert
	@# mit "Unable to start the app on the device" (gemessen 2026-09-07).
	@#
	@# Eine Datei faehrt hier nicht mit: `m2_first_decision_test.dart` ist der
	@# Bildschirm-Treiber von `tests/e2e/m2_first_decision/run.sh` (HUM-097) und
	@# verlangt Daemon, Agent und drei Dateipfade in der Umgebung; ohne sie
	@# stirbt er sofort mit "HUMANITL_E2E_HAR is not set". Sein Gate ist der Job
	@# `e2e-xvfb`, der `run.sh` faehrt. Uebersprungen wird laut, nicht still.
	cd app && for file in integration_test/*_test.dart; do \
		case "$$file" in \
		*/m2_first_decision_test.dart) \
			if [ -z "$$HUMANITL_E2E_HAR" ]; then \
				echo "== $$file SKIPPED: driven by tests/e2e/m2_first_decision/run.sh (HUMANITL_E2E_HAR is unset)"; \
				continue; \
			fi ;; \
		esac; \
		echo "== $$file"; \
		flutter test "$$file" -d linux || exit 1; \
	done

# flutter-analyze and flutter-test depend on this: app/lib/core/ipc/generated/
# is gitignored and imported by the app. Without protoc or protoc-gen-dart the
# script skips the Dart half and exits 0 (with STRICT=1 or CI=true it exits 1),
# so `make check` keeps working on a machine without them.
proto: ## Regenerate protobuf code for Rust and Dart (HUM-003)
	@export PATH="$(TOOLS_PATH)"; ./scripts/gen-proto.sh

escape: ## Run the sandbox escape tests (HUM-006)
	./tests/escape/run.sh

e2e: ## Run the demo scripts of every milestone reached (E2E_ONLY=m1|m2|m3 picks one)
	./tests/e2e/run.sh

clean: ## Remove build artefacts
	cd daemon && cargo clean
	cd app && flutter clean
