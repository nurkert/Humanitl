Closes #

## What was built

<!-- Two to four sentences: what changed and why. One issue per pull request. -->

## Screenshots

<!-- UI changes only: before and after, light and dark. Delete this section otherwise. -->

## Author checklist

- [ ] Targeted checks green: tests of the touched crates and packages, `cargo fmt`, `cargo clippy -D warnings`, `make flutter-analyze` for app changes
- [ ] Mutation proofs named in the issue run: each mutation turns its test red
- [ ] Security statement in `README.md` touched? Then `docs/SECURITY.md` and `docs/THREAT-MODEL.md` change in this pull request
- [ ] New UI strings in `app/l10n/app_en.arb` and `app_de.arb`
- [ ] Coupling ratchet holds (`python3 tools/check_coupling.py`); a falling count updates the baseline here
- [ ] Docs updated where behaviour, settings or commands changed
- [ ] No change to `daemon/Cargo.toml` from an agent

## Review

<!-- Codex and Antigravity findings, one line each: severity, finding, taken or not taken, and a one-sentence reason. -->
