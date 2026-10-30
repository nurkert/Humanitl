---
source: reference/parity.md
source-sha256: c250ea701e275e14fcd193011df01b3ad6c9a73683eee29ec13258ebec639e5f
lang: en
---
# Parity table

Applies to: Humanitl 0.1

<!-- Erzeugt von `cargo xtask docs` (HUM-078). Nicht von Hand ändern. -->

Every capability is an RPC first; command line and interface are thin
clients of the same proto (ADR-018). The table puts every method of the
service next to its subcommand and its place in the interface.

Sources:

- RPC: the service methods under `proto/humanitl/v1/`;
- CLI: `PARITY` in `daemon/bin/humanitl/src/parity.rs`;
- UI: `parity` in `app/lib/core/parity.dart`, path below `app/lib/`;
- Exemptions: `daemon/xtask/parity_exempt.toml`.

An RPC without a CLI row and without an exemption breaks the CI job
`parity-check`; an RPC without a place in the interface is a warning.

| RPC | CLI | UI |
|---|---|---|
| `Humanitl.Audit` | `humanitl audit export`<br>`humanitl audit verify` | `features/audit/audit_screen` |
| `Humanitl.Decide` | `humanitl flows decide` | `features/intercept/widgets/action_bar` |
| `Humanitl.DiscoverLlm` | `humanitl llm discover` | `features/setup/widgets/llm_discover_sheet` |
| `Humanitl.Doctor` | `humanitl doctor` | `features/setup/widgets/doctor_list` |
| `Humanitl.Exec` | `humanitl sandbox exec` | `features/sandbox/widgets/terminal_tabs` |
| `Humanitl.GetBody` | `humanitl flows show --body` | `features/history/history_detail` |
| `Humanitl.GetConfig` | Exemption | `features/settings/settings_sheet` |
| `Humanitl.GetFlow` | `humanitl flows show` | `features/history/history_detail` |
| `Humanitl.GetInfo` | `humanitl daemon status` | `features/shell/connection_gate` |
| `Humanitl.GetSessionSummary` | `humanitl sessions summary` | missing |
| `Humanitl.ListFlows` | `humanitl flows list` | `features/history/history_screen` |
| `Humanitl.LlmKey` | `humanitl llm key remove`<br>`humanitl llm key set`<br>`humanitl llm key status` | `features/settings/widgets/model_section` |
| `Humanitl.LlmServers` | `humanitl llm server add`<br>`humanitl llm server ls`<br>`humanitl llm server rename`<br>`humanitl llm server rm`<br>`humanitl llm server use` | `features/settings/widgets/server_switcher` |
| `Humanitl.ProbeLlm` | `humanitl llm test` | `features/setup/widgets/llm_check` |
| `Humanitl.Rules` | `humanitl rules add`<br>`humanitl rules disable`<br>`humanitl rules dry-run`<br>`humanitl rules enable`<br>`humanitl rules list`<br>`humanitl rules reload`<br>`humanitl rules remove`<br>`humanitl rules reorder`<br>`humanitl rules test`<br>`humanitl rules update` | `features/rules/rules_screen` |
| `Humanitl.Sandbox` | `humanitl run`<br>`humanitl run --disposable`<br>`humanitl sandbox apply`<br>`humanitl sandbox changes`<br>`humanitl sandbox discard` | `features/sandbox/sandbox_screen` |
| `Humanitl.SetConfig` | Exemption | `core/ui/fix_control` |
| `Humanitl.Subscribe` | `humanitl run --ask terminal` | `features/intercept/widgets/queue_pane` |
| `Humanitl.Terminal` | `humanitl sandbox attach`<br>`humanitl sandbox attach --shell` | `features/sandbox/widgets/terminal_pane` |

## UI gaps

- `Humanitl.GetSessionSummary`

## Exemptions

| RPC | Reason |
|---|---|
| `Humanitl.GetConfig` | The CLI reads and writes the configuration locally through the same loader as the daemon; the interface has used both RPCs since HUM-069, the command-line connection follows with HUM-170. |
| `Humanitl.SetConfig` | The CLI reads and writes the configuration locally through the same loader as the daemon; the interface has used both RPCs since HUM-069, the command-line connection follows with HUM-170. |
