/// Wo die Oberfläche welche RPC des Daemons bedient (ADR-018, HUM-078).
///
/// `cargo xtask docs` liest diese Datei als Text, nicht als Dart, und erzeugt
/// daraus die Spalte „UI" von `docs/reference/parity.md`. Das Format ist
/// deshalb streng: eine Zeile je RPC, genau `'Humanitl.<Rpc>': '<ort>',`,
/// Kommentare nur als eigene Zeile. Der Ort ist der Pfad der Datei unter
/// `app/lib/` ohne `.dart`, und die Datei muss es geben; ein verschobenes
/// Widget bricht den CI-Job `parity-check`, bis die Zeile nachgezogen ist.
///
/// Eine RPC ohne Zeile ist erlaubt und erscheint als Warnung und im Abschnitt
/// „UI-Lücken" der Tabelle: Die Oberfläche darf einen Sprint hinter der
/// Kommandozeile sein, nicht länger.
library;

/// RPC-Name (`Service.Methode`) → Ort in der Oberfläche.
const parity = <String, String>{
  'Humanitl.Audit': 'features/audit/audit_screen',
  'Humanitl.Decide': 'features/intercept/widgets/action_bar',
  'Humanitl.DiscoverLlm': 'features/setup/widgets/llm_discover_sheet',
  'Humanitl.Doctor': 'features/setup/widgets/doctor_list',
  'Humanitl.GetBody': 'features/history/history_detail',
  'Humanitl.GetFlow': 'features/history/history_detail',
  'Humanitl.GetInfo': 'features/shell/connection_gate',
  'Humanitl.ListFlows': 'features/history/history_screen',
  'Humanitl.ProbeLlm': 'features/setup/widgets/llm_check',
  'Humanitl.Rules': 'features/rules/rules_screen',
  'Humanitl.Sandbox': 'features/sandbox/sandbox_screen',
  // Die Knöpfe unter einem Befund, die einen Wert in `config.toml` setzen.
  'Humanitl.SetConfig': 'core/ui/fix_control',
  'Humanitl.Subscribe': 'features/intercept/widgets/queue_pane',
  'Humanitl.Terminal': 'features/sandbox/widgets/terminal_pane',
};
