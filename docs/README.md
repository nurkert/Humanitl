# Documentation

Applies to: Humanitl 0.1

Where to find what. Pages marked *generated* come from the code through `cargo xtask docs`; CI
fails when they are stale.

## Using Humanitl

| Page | What it answers |
|---|---|
| [`guides/install.md`](guides/install.md) | prerequisites, `.deb`, AppImage, the user service, checks, removal |
| [`guides/first-session.md`](guides/first-session.md) | from the setup screen to the first held request, a rule and the audit check |
| [`guides/agent-profiles.md`](guides/agent-profiles.md) | session profiles, the OpenCode adapter, writing an adapter, phone-home hosts |

## Reference

| Page | What it answers |
|---|---|
| [`reference/rules.md`](reference/rules.md) | every rule field, matching semantics, evaluation order, recipes, bundled rules |
| [`reference/cli.md`](reference/cli.md) | every command and flag (*generated*) |
| [`CONFIG.md`](CONFIG.md) | every configuration key with type, default, tier and project scope (*generated*, German) |
| [`DIAGNOSTICS.md`](DIAGNOSTICS.md) | every diagnostic code with trigger and fix (*generated*, German) |
| [`reference/limits.md`](reference/limits.md) | the resource limits and `stream: true` |
| [`reference/parity.md`](reference/parity.md) | which RPC each CLI command and screen uses (*generated*) |
| [`cli.md`](cli.md) | what `humanitl run`, `doctor`, `config`, `audit` and `daemon` do step by step |
| [`PROTOCOL.md`](PROTOCOL.md) | the gRPC contract |
| [`GLOSSARY.md`](GLOSSARY.md) | terms in German and English |

## Security

| Page | What it answers |
|---|---|
| [`SECURITY.md`](SECURITY.md) | the claim, the three guarantees, side channels, proxy, rules, audit, reporting |
| [`THREAT-MODEL.md`](THREAT-MODEL.md) | assets, attackers, trust boundaries, attack surface, residual risks |

## Building Humanitl

| Page | What it answers |
|---|---|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | layers, crates and the dependency rules CI enforces |
| [`DESIGN.md`](DESIGN.md) | the "Airlock" design system: tokens, components, motion |
| [`UX.md`](UX.md) | screens and flows |
| [`INSTALL.md`](INSTALL.md) | maintainer notes: APT repository, unit hardening, packaging, test matrix |
| [`adr/README.md`](adr/README.md) | architecture decision records |
