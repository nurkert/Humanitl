# Humanitl — developer entry points.
# Everything CI runs is reachable from here. `make check` is the gate.

SHELL := /bin/bash
.DEFAULT_GOAL := help

.PHONY: help check rust-fmt rust-clippy rust-build rust-test rust-doc rust-deny typed-errors-lint \
        flutter-get flutter-analyze flutter-test flutter-test-dbus flutter-test-daemon \
        flutter-test-integration flutter-build runner-test proto escape e2e \
        deps-lint docs-lint parity-check l10n-lint catalog-assets catalog-lint clean package

help: ## List targets
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) | sort | awk -F':.*?## ' '{printf "  %-18s %s\n", $$1, $$2}'

check: rust-fmt rust-clippy rust-build rust-test rust-doc deps-lint docs-lint parity-check typed-errors-lint catalog-lint l10n-lint flutter-analyze flutter-test runner-test flutter-build ## Full local gate (same steps as CI)

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

# Die Sprachprüfung braucht nur `dart` und keine erzeugten Dateien (HUM-052): ARB-Parität,
# Literale in den Features, Titel und Grund jedes Diagnose-Codes, das Glossar.
l10n-lint: flutter-get ## Localization lint: ARB parity, literals, diagnostic codes, glossary (HUM-052)
	cd app && dart run tool/l10n_lint.dart

docs-lint: ## Check the security documents (HUM-007)
	./scripts/ci/lint-docs.sh

parity-check: ## Every RPC has a CLI subcommand; docs/reference/parity.md is current (HUM-078)
	./scripts/ci/parity-check.sh

flutter-get: ## Fetch Dart packages (app and packages/ui)
	cd app && flutter pub get
	cd app/packages/ui && flutter pub get

# Der erzeugte Code ist keine Quelle (ARCHITECTURE 4) und steht deshalb nicht im
# Repository. Analyse, Test und Bau haengen daran: In einem frischen Auscheckstand
# fehlen sonst alle .g.dart- und .freezed.dart-Dateien, und `flutter analyze`
# meldet Hunderte Fehler, die keine sind.
flutter-codegen: flutter-get proto catalog-assets ## Generated Dart code: riverpod, freezed, ARB
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

# Der einzige C++-Test des Repositories. Er haengt an nichts ausser POSIX --
# kein GTK, kein Flutter, keine Ephemeral-Header --, laeuft in unter einer
# Sekunde und misst, was `flutter test` nicht erreicht: das Protokoll, das der
# Runner beim Signal und beim Absturz schreibt (HUM-136).
runner-test: ## Test des Runner-Protokolls (app/linux/runner)
	app/linux/runner/run_exit_log_test.sh

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
flutter-test-daemon: flutter-codegen ## Sandbox and audit screens against a real daemon (HUM-040, HUM-156)
	@test -x daemon/target/debug/humanitld || { echo "daemon/target/debug/humanitld missing: cargo build --manifest-path daemon/Cargo.toml" >&2; exit 1; }
	@command -v bwrap >/dev/null 2>&1 || { echo "bwrap missing: install the bubblewrap package" >&2; exit 1; }
	cd app && env HUMANITL_DAEMON_TESTS=1 flutter test test/features/sandbox/daemon_live_test.dart test/features/audit/audit_daemon_live_test.dart

# Die Integrationstests: die echte Anwendung auf einem Bildschirm, gegen den
# echten Daemon. Nicht Teil von `check`, aus denselben zwei Gruenden wie die
# beiden Ziele darueber: Sie brauchen einen Bildschirm (`Xvfb :99` genuegt) und
# starten Prozesse. Wer sie faehrt, prueft damit die Naht, die kein
# Widget-Test sieht -- was die Oberflaeche zeigt, wenn ein echter Dienst
# antwortet (HUM-029, HUM-097).

# Wohin `flutter-test-integration` Testausgabe und Daemon-Log legt; der Job
# `e2e-xvfb` laedt `target/e2e` als Artefakt hoch (HUM-185).
INTEGRATION_LOGS := $(CURDIR)/target/e2e/integration

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
	@#
	@# Jede Datei schreibt ihre Ausgabe zusaetzlich nach
	@# `target/e2e/integration/<datei>.log`, und `queue_freeze_test.dart` legt
	@# das Log seines Daemons daneben (`HUMANITL_INTEGRATION_LOGS`). Der Job
	@# `e2e-xvfb` laedt `target/e2e` als Artefakt hoch, auch nach einem roten
	@# Schritt; sein eigenes Log ist ohne Admin-Rechte nicht lesbar (HUM-185).
	rm -rf "$(INTEGRATION_LOGS)" && mkdir -p "$(INTEGRATION_LOGS)"
	cd app && export HUMANITL_INTEGRATION_LOGS="$(INTEGRATION_LOGS)" && \
	for file in integration_test/*_test.dart; do \
		case "$$file" in \
		*/m2_first_decision_test.dart) \
			if [ -z "$$HUMANITL_E2E_HAR" ]; then \
				echo "== $$file SKIPPED: driven by tests/e2e/m2_first_decision/run.sh (HUMANITL_E2E_HAR is unset)"; \
				continue; \
			fi ;; \
		esac; \
		echo "== $$file"; \
		log="$$HUMANITL_INTEGRATION_LOGS/$$(basename "$$file" .dart).log"; \
		flutter test "$$file" -d linux 2>&1 | tee "$$log"; \
		status=$${PIPESTATUS[0]}; \
		if [ "$$status" -ne 0 ]; then \
			echo "== $$file failed with $$status; output in $$log, logs in $$HUMANITL_INTEGRATION_LOGS" >&2; \
			exit "$$status"; \
		fi; \
	done

# flutter-analyze and flutter-test depend on this: app/lib/core/ipc/generated/
# is gitignored and imported by the app. Without protoc or protoc-gen-dart the
# script skips the Dart half and exits 0 (with STRICT=1 or CI=true it exits 1),
# so `make check` keeps working on a machine without them.
proto: ## Regenerate protobuf code for Rust and Dart (HUM-003)
	@export PATH="$(TOOLS_PATH)"; ./scripts/gen-proto.sh

# Pakete aus einem Release-Bau (HUM-053): `make package PACKAGE_VERSION=0.0.N`
# legt `dist/humanitl_<ver>_amd64.deb` und `dist/Humanitl-<ver>-x86_64.AppImage`
# ab. Die Reihenfolge ist die der Spezifikation: erst die drei Programme nach
# app/linux/bundle-extra/, dann das Flutter-Bundle (CMakeLists.txt legt sie nach
# bin/), dann die Pakete. Die Version kommt aus dem Aufruf und wird in den
# Skripten gegen ^0\.0\.N$ geprueft; in die Quelltexte stempelt sie nur der
# Release-Lauf (packaging/release/stamp-version.sh), hier steht, was im
# Auscheckstand steht.
PACKAGE_VERSION ?= 0.0.0
PACKAGE_BUNDLE := app/build/linux/x64/release/bundle

package: flutter-codegen ## .deb and AppImage from a release build (HUM-053), PACKAGE_VERSION=0.0.N
	@export PATH="$(TOOLS_PATH)"; set -euo pipefail; \
	./packaging/release/build-binaries.sh app/linux/bundle-extra; \
	(cd app && flutter build linux --release --build-name "$(PACKAGE_VERSION)"); \
	./packaging/release/check-version.sh "$(PACKAGE_VERSION)" "$(PACKAGE_BUNDLE)/bin" "$(PACKAGE_BUNDLE)"; \
	./packaging/deb/build-deb.sh "$(PACKAGE_VERSION)" "$(PACKAGE_BUNDLE)/bin" "$(PACKAGE_BUNDLE)" dist; \
	./packaging/appimage/build-appimage.sh "$(PACKAGE_VERSION)" "$(PACKAGE_BUNDLE)/bin" "$(PACKAGE_BUNDLE)" dist; \
	./packaging/appimage/check-appimage.sh "dist/Humanitl-$(PACKAGE_VERSION)-x86_64.AppImage" "$(PACKAGE_VERSION)"; \
	ls -l dist

escape: ## Run the sandbox escape tests (HUM-006)
	./tests/escape/run.sh

e2e: ## Run the demo scripts of every milestone reached (E2E_ONLY=m1|m2|m3 picks one)
	./tests/e2e/run.sh

# Der Katalog ist eine Quelle, die Kopie unter `app/assets/` ist es nicht
# (ARCHITECTURE 4). Sie steht trotzdem im Repository, weil ein Auscheckstand
# ohne sie nicht baut: `flutter build` liest den Asset-Block der pubspec und
# bricht ab, wenn eine gelistete Datei fehlt. Damit sie nie driftet, erzeugt
# `catalog-assets` sie und `catalog-lint` haelt sie Byte fuer Byte gegen das
# Original; driftet sie doch, nennt der Bildschirm einen anderen Dienst, als
# der Daemon zugeordnet hat (HUM-094).
catalog-assets: ## Copy the domain catalog into the Flutter asset bundle
	install -D -m 0644 catalog/domains.yaml app/assets/catalog/domains.yaml

catalog-lint: ## The bundled catalog matches the source byte for byte
	@cmp -s catalog/domains.yaml app/assets/catalog/domains.yaml || \
	  { echo "app/assets/catalog/domains.yaml differs from catalog/domains.yaml; run make catalog-assets" >&2; exit 1; }
	@echo "catalog-lint: app/assets/catalog/domains.yaml matches catalog/domains.yaml"

clean: ## Remove build artefacts
	cd daemon && cargo clean
	cd app && flutter clean
