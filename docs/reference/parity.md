# Paritäts-Tabelle

Applies to: Humanitl 0.1

<!-- Erzeugt von `cargo xtask docs` (HUM-078). Nicht von Hand ändern. -->

Jede Fähigkeit ist zuerst eine RPC; Kommandozeile und Oberfläche sind
dünne Clients derselben Proto (ADR-018). Die Tabelle stellt jede Methode
des Dienstes neben ihr Unterkommando und ihren Ort in der Oberfläche.

Quellen:

- RPC: die Service-Methoden unter `proto/humanitl/v1/`;
- CLI: `PARITY` in `daemon/bin/humanitl/src/parity.rs`;
- UI: `parity` in `app/lib/core/parity.dart`, Pfad unter `app/lib/`;
- Ausnahmen: `daemon/xtask/parity_exempt.toml`.

Eine RPC ohne CLI-Zeile und ohne Ausnahme bricht den CI-Job
`parity-check`; eine RPC ohne Ort in der Oberfläche ist eine Warnung.

| RPC | CLI | UI |
|---|---|---|
| `Humanitl.Audit` | `humanitl audit export`<br>`humanitl audit verify` | `features/audit/audit_screen` |
| `Humanitl.Decide` | `humanitl flows decide` | `features/intercept/widgets/action_bar` |
| `Humanitl.DiscoverLlm` | `humanitl llm discover` | `features/setup/widgets/llm_discover_sheet` |
| `Humanitl.Doctor` | `humanitl doctor` | `features/setup/widgets/doctor_list` |
| `Humanitl.Exec` | `humanitl sandbox exec` | `features/sandbox/widgets/terminal_tabs` |
| `Humanitl.GetBody` | `humanitl flows show --body` | `features/history/history_detail` |
| `Humanitl.GetConfig` | Ausnahme | `features/settings/settings_sheet` |
| `Humanitl.GetFlow` | `humanitl flows show` | `features/history/history_detail` |
| `Humanitl.GetInfo` | `humanitl daemon status` | `features/shell/connection_gate` |
| `Humanitl.GetSessionSummary` | `humanitl sessions summary` | fehlt |
| `Humanitl.ListFlows` | `humanitl flows list` | `features/history/history_screen` |
| `Humanitl.LlmKey` | `humanitl llm key remove`<br>`humanitl llm key set`<br>`humanitl llm key status` | `features/settings/widgets/model_section` |
| `Humanitl.LlmServers` | `humanitl llm server add`<br>`humanitl llm server ls`<br>`humanitl llm server rename`<br>`humanitl llm server rm`<br>`humanitl llm server use` | `features/settings/widgets/server_switcher` |
| `Humanitl.ProbeLlm` | `humanitl llm test` | `features/setup/widgets/llm_check` |
| `Humanitl.Rules` | `humanitl rules add`<br>`humanitl rules disable`<br>`humanitl rules dry-run`<br>`humanitl rules enable`<br>`humanitl rules list`<br>`humanitl rules reload`<br>`humanitl rules remove`<br>`humanitl rules reorder`<br>`humanitl rules test`<br>`humanitl rules update` | `features/rules/rules_screen` |
| `Humanitl.Sandbox` | `humanitl run`<br>`humanitl run --direct`<br>`humanitl run --disposable`<br>`humanitl sandbox apply`<br>`humanitl sandbox changes`<br>`humanitl sandbox discard`<br>`humanitl sandbox plan` | `features/sandbox/sandbox_screen` |
| `Humanitl.SetConfig` | Ausnahme | `core/ui/fix_control` |
| `Humanitl.Subscribe` | `humanitl run --ask terminal` | `features/intercept/widgets/queue_pane` |
| `Humanitl.Terminal` | `humanitl sandbox attach`<br>`humanitl sandbox attach --shell` | `features/sandbox/widgets/terminal_pane` |

## UI-Lücken

- `Humanitl.GetSessionSummary`

## Ausnahmen

| RPC | Begründung |
|---|---|
| `Humanitl.GetConfig` | CLI liest und schreibt die Konfiguration lokal über denselben Lader wie der Daemon; die Oberfläche nutzt beide RPCs seit HUM-069, die Anbindung der Kommandozeile folgt mit HUM-170. |
| `Humanitl.SetConfig` | CLI liest und schreibt die Konfiguration lokal über denselben Lader wie der Daemon; die Oberfläche nutzt beide RPCs seit HUM-069, die Anbindung der Kommandozeile folgt mit HUM-170. |
