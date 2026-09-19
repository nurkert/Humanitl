# Paritäts-Tabelle

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
| `Humanitl.GetBody` | `humanitl flows show --body` | `features/history/history_detail` |
| `Humanitl.GetConfig` | Ausnahme | fehlt |
| `Humanitl.GetFlow` | `humanitl flows show` | `features/history/history_detail` |
| `Humanitl.GetInfo` | `humanitl daemon status` | `features/shell/connection_gate` |
| `Humanitl.GetSessionSummary` | `humanitl sessions summary` | fehlt |
| `Humanitl.ListFlows` | `humanitl flows list` | `features/history/history_screen` |
| `Humanitl.ProbeLlm` | `humanitl llm test` | `features/setup/widgets/llm_check` |
| `Humanitl.Rules` | `humanitl rules add`<br>`humanitl rules disable`<br>`humanitl rules dry-run`<br>`humanitl rules enable`<br>`humanitl rules list`<br>`humanitl rules reload`<br>`humanitl rules remove`<br>`humanitl rules reorder`<br>`humanitl rules test`<br>`humanitl rules update` | `features/rules/rules_screen` |
| `Humanitl.Sandbox` | `humanitl run` | `features/sandbox/sandbox_screen` |
| `Humanitl.SetConfig` | Ausnahme | `core/ui/fix_control` |
| `Humanitl.Subscribe` | `humanitl run --ask terminal` | `features/intercept/widgets/queue_pane` |
| `Humanitl.Terminal` | `humanitl sandbox attach` | `features/sandbox/widgets/terminal_pane` |

## UI-Lücken

- `Humanitl.GetConfig`
- `Humanitl.GetSessionSummary`

## Ausnahmen

| RPC | Begründung |
|---|---|
| `Humanitl.GetConfig` | CLI liest und schreibt die Konfiguration lokal über denselben Lader wie der Daemon; eine echte Anbindung an GetConfig/SetConfig folgt mit HUM-069/HUM-170. |
| `Humanitl.SetConfig` | CLI liest und schreibt die Konfiguration lokal über denselben Lader wie der Daemon; eine echte Anbindung an GetConfig/SetConfig folgt mit HUM-069/HUM-170. |
