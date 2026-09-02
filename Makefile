# Humanitl — developer entry points.
# Everything CI runs is reachable from here. `make check` is the gate.

SHELL := /bin/bash
.DEFAULT_GOAL := help

.PHONY: help check rust-fmt rust-clippy rust-build rust-test rust-deny \
        flutter-get flutter-analyze flutter-test proto escape e2e deps-lint clean

help: ## List targets
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) | sort | awk -F':.*?## ' '{printf "  %-18s %s\n", $$1, $$2}'

check: rust-fmt rust-clippy rust-build rust-test deps-lint flutter-analyze flutter-test ## Full local gate

rust-fmt: ## cargo fmt --check (skipped when rustfmt is absent)
	@if cd daemon && cargo fmt --version >/dev/null 2>&1; then cargo fmt --all -- --check; \
	elif [[ -n "$$STRICT" ]]; then echo "rustfmt missing and STRICT set" >&2; exit 1; \
	else echo "SKIP rust-fmt: rustfmt component not installed (rustup component add rustfmt)"; fi

rust-clippy: ## cargo clippy -D warnings (skipped when clippy is absent)
	@if cd daemon && cargo clippy --version >/dev/null 2>&1; then cargo clippy --workspace --all-targets -- -D warnings; \
	elif [[ -n "$$STRICT" ]]; then echo "clippy missing and STRICT set" >&2; exit 1; \
	else echo "SKIP rust-clippy: clippy component not installed (rustup component add clippy)"; fi

rust-build: ## Build the whole daemon workspace
	cd daemon && cargo build --workspace --all-targets

rust-test: ## Run all Rust tests
	cd daemon && cargo test --workspace

rust-deny: ## License and advisory audit (needs cargo-deny)
	cd daemon && cargo deny check

deps-lint: ## Enforce the dependency direction (HUM-074)
	./tools/check-deps.sh
	python3 tools/tests/check_deps_test.py

flutter-get: ## Fetch Dart packages
	cd app && flutter pub get

flutter-analyze: flutter-get ## Static analysis of the Flutter app
	cd app && flutter analyze

flutter-test: flutter-get ## Flutter unit and widget tests
	cd app && flutter test

proto: ## Regenerate protobuf code for Rust and Dart (HUM-003)
	./scripts/gen-proto.sh

escape: ## Run the sandbox escape tests (HUM-006)
	./tests/escape/run.sh

e2e: ## Run the end-to-end demo script of the current milestone
	./tests/e2e/run.sh

clean: ## Remove build artefacts
	cd daemon && cargo clean
	cd app && flutter clean
