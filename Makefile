# Humanitl — developer entry points.
# Everything CI runs is reachable from here. `make check` is the gate.

SHELL := /bin/bash
.DEFAULT_GOAL := help

.PHONY: help check rust-fmt rust-clippy rust-build rust-test rust-doc rust-deny typed-errors-lint \
        flutter-get flutter-analyze flutter-test flutter-build proto escape e2e deps-lint docs-lint clean

help: ## List targets
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) | sort | awk -F':.*?## ' '{printf "  %-18s %s\n", $$1, $$2}'

check: rust-fmt rust-clippy rust-build rust-test rust-doc deps-lint docs-lint typed-errors-lint flutter-analyze flutter-test flutter-build ## Full local gate (same steps as CI)

# A rustup toolchain may exist without rustup on PATH (this machine): put its
# bin directory first so `cargo fmt` and `cargo clippy` find their components.
RUSTUP_BIN := $(firstword $(wildcard $(HOME)/.rustup/toolchains/*/bin))
TOOLS_PATH := $(if $(RUSTUP_BIN),$(RUSTUP_BIN):)$(PATH)

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

typed-errors-lint: ## No public daemon signature returns String or anyhow errors (HUM-063)
	scripts/ci/lint-no-string-errors.sh --self-test
	scripts/ci/lint-no-string-errors.sh

deps-lint: ## Enforce the dependency direction (HUM-074)
	./tools/check-deps.sh
	python3 tools/tests/check_deps_test.py

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

flutter-build: flutter-codegen ## Debug build of the Linux desktop app (CI parity)
	cd app && flutter build linux --debug

flutter-test: flutter-codegen ## Flutter unit and widget tests (app and packages/ui)
	cd app && flutter test
	cd app/packages/ui && flutter test

# flutter-analyze and flutter-test depend on this: app/lib/core/ipc/generated/
# is gitignored and imported by the app. Without protoc or protoc-gen-dart the
# script skips the Dart half and exits 0 (with STRICT=1 or CI=true it exits 1),
# so `make check` keeps working on a machine without them.
proto: ## Regenerate protobuf code for Rust and Dart (HUM-003)
	./scripts/gen-proto.sh

escape: ## Run the sandbox escape tests (HUM-006)
	./tests/escape/run.sh

e2e: ## Run the end-to-end demo script of the current milestone
	./tests/e2e/run.sh

clean: ## Remove build artefacts
	cd daemon && cargo clean
	cd app && flutter clean
