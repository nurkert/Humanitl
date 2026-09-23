/// Title and cause of a diagnostic in the person's language (HUM-052).
///
/// Der Daemon liefert zu jedem Befund einen `code`, die Überschrift aus seinem
/// Register und einen Satz `why`. Die Überschrift ist Deutsch, der Satz meist
/// Englisch, und keins von beiden folgt der Sprache der Oberfläche. Übersetzt
/// wird deshalb anhand des Codes: `diag<CODE>Title` und `diag<CODE>Why` in
/// `l10n/app_*.arb`, der Code ohne Unterstrich (`diagSANDBOX001Title`).
///
/// Zwei Regeln bestimmen, was auf dem Schirm steht:
///
/// - Die Überschrift kommt aus ARB, sobald es den Schlüssel gibt. Sie ist der
///   feste Teil der Meldung und verliert beim Übersetzen nichts.
/// - Der Grund ist der gemessene Satz des Daemons, wo er einen mitschickt
///   (`docs/UX.md` 4.4): Er nennt Pfad, Host und Fehlertext, und ein
///   übersetzter Satz ohne diese Angaben wäre ärmer. Der übersetzte
///   Grundsatz steht nur dort, wo der Befund keinen eigenen Satz trägt,
///   etwa bei einem Befund, den der Client selbst baut.
///
/// Ein Code, den diese Fassung noch nicht kennt, behält die Worte des Daemons.
///
/// Ein eigenes Fix-Label je Code gibt es nicht: Den Knopf beschriftet
/// `FixControl` nach der Art der `FixAction`, und diese Beschriftungen stehen
/// schon in ARB.
library;

import '../core/domain/domain.dart';
import 'generated/app_localizations.dart';

/// Title and causes of one diagnostic, already in the person's language.
class DiagnosticText {
  /// Creates the texts.
  const DiagnosticText({
    required this.title,
    required this.why,
    required this.cause,
  });

  /// The fixed part of the message: `diag<CODE>Title`, else the daemon's
  /// title, else the code itself.
  final String title;

  /// The generic cause: `diag<CODE>Why`, else the daemon's own sentence.
  final String why;

  /// What stands in the cause slot of a card: the daemon's own sentence
  /// when there is one, else [why] (`docs/UX.md` 4.4).
  final String cause;
}

/// Resolves the texts of a [Diagnostic] from the ARB files.
abstract final class DiagnosticL10n {
  /// The texts of [diagnostic] in the language of [l10n].
  static DiagnosticText resolve(Diagnostic diagnostic, AppLocalizations l10n) {
    final (String, String)? own = diagnosticTexts(l10n, diagnostic.code);
    final String why = own?.$2 ?? diagnostic.why;
    return DiagnosticText(
      title:
          own?.$1 ??
          (diagnostic.title.isEmpty ? diagnostic.code : diagnostic.title),
      why: why,
      cause: diagnostic.why.isEmpty ? why : diagnostic.why,
    );
  }
}

/// Title and generic cause of [code], or `null` for a code this build does
/// not know.
///
/// One line per code of the register
/// (`daemon/crates/core-types/src/diagnostics/codes.rs`). A code added there
/// needs its two ARB keys (`tool/l10n_lint.dart` checks that) and its line
/// here (`test/l10n/diagnostic_l10n_test.dart` checks that).
(String, String)? diagnosticTexts(AppLocalizations l10n, String code) =>
    switch (code) {
      'AGENT_001' => (l10n.diagAGENT001Title, l10n.diagAGENT001Why),
      'AGENT_002' => (l10n.diagAGENT002Title, l10n.diagAGENT002Why),
      'AGENT_003' => (l10n.diagAGENT003Title, l10n.diagAGENT003Why),
      'AGENT_004' => (l10n.diagAGENT004Title, l10n.diagAGENT004Why),
      'AGENT_005' => (l10n.diagAGENT005Title, l10n.diagAGENT005Why),
      'AUDIT_001' => (l10n.diagAUDIT001Title, l10n.diagAUDIT001Why),
      'AUDIT_002' => (l10n.diagAUDIT002Title, l10n.diagAUDIT002Why),
      'AUDIT_003' => (l10n.diagAUDIT003Title, l10n.diagAUDIT003Why),
      'AUDIT_004' => (l10n.diagAUDIT004Title, l10n.diagAUDIT004Why),
      'AUDIT_005' => (l10n.diagAUDIT005Title, l10n.diagAUDIT005Why),
      'AUDIT_006' => (l10n.diagAUDIT006Title, l10n.diagAUDIT006Why),
      'AUDIT_007' => (l10n.diagAUDIT007Title, l10n.diagAUDIT007Why),
      'AUDIT_008' => (l10n.diagAUDIT008Title, l10n.diagAUDIT008Why),
      'AUDIT_009' => (l10n.diagAUDIT009Title, l10n.diagAUDIT009Why),
      'CATALOG_001' => (l10n.diagCATALOG001Title, l10n.diagCATALOG001Why),
      'CATALOG_002' => (l10n.diagCATALOG002Title, l10n.diagCATALOG002Why),
      'CLI_001' => (l10n.diagCLI001Title, l10n.diagCLI001Why),
      'CLI_002' => (l10n.diagCLI002Title, l10n.diagCLI002Why),
      'CLI_003' => (l10n.diagCLI003Title, l10n.diagCLI003Why),
      'CLI_004' => (l10n.diagCLI004Title, l10n.diagCLI004Why),
      'CLI_005' => (l10n.diagCLI005Title, l10n.diagCLI005Why),
      'CLI_006' => (l10n.diagCLI006Title, l10n.diagCLI006Why),
      'CONFIG_001' => (l10n.diagCONFIG001Title, l10n.diagCONFIG001Why),
      'CONFIG_002' => (l10n.diagCONFIG002Title, l10n.diagCONFIG002Why),
      'CONFIG_003' => (l10n.diagCONFIG003Title, l10n.diagCONFIG003Why),
      'CONFIG_004' => (l10n.diagCONFIG004Title, l10n.diagCONFIG004Why),
      'CONFIG_005' => (l10n.diagCONFIG005Title, l10n.diagCONFIG005Why),
      'CONFIG_006' => (l10n.diagCONFIG006Title, l10n.diagCONFIG006Why),
      'CONFIG_007' => (l10n.diagCONFIG007Title, l10n.diagCONFIG007Why),
      'CONFIG_008' => (l10n.diagCONFIG008Title, l10n.diagCONFIG008Why),
      'CONFIG_009' => (l10n.diagCONFIG009Title, l10n.diagCONFIG009Why),
      'CONFIG_010' => (l10n.diagCONFIG010Title, l10n.diagCONFIG010Why),
      'CONFIG_011' => (l10n.diagCONFIG011Title, l10n.diagCONFIG011Why),
      'CONFIG_012' => (l10n.diagCONFIG012Title, l10n.diagCONFIG012Why),
      'CONFIG_013' => (l10n.diagCONFIG013Title, l10n.diagCONFIG013Why),
      'CONFIG_014' => (l10n.diagCONFIG014Title, l10n.diagCONFIG014Why),
      'CONFIG_015' => (l10n.diagCONFIG015Title, l10n.diagCONFIG015Why),
      'CONFIG_016' => (l10n.diagCONFIG016Title, l10n.diagCONFIG016Why),
      'CONFIG_017' => (l10n.diagCONFIG017Title, l10n.diagCONFIG017Why),
      'CONFIG_018' => (l10n.diagCONFIG018Title, l10n.diagCONFIG018Why),
      'DAEMON_001' => (l10n.diagDAEMON001Title, l10n.diagDAEMON001Why),
      'DAEMON_002' => (l10n.diagDAEMON002Title, l10n.diagDAEMON002Why),
      'DAEMON_003' => (l10n.diagDAEMON003Title, l10n.diagDAEMON003Why),
      'DAEMON_004' => (l10n.diagDAEMON004Title, l10n.diagDAEMON004Why),
      'DAEMON_005' => (l10n.diagDAEMON005Title, l10n.diagDAEMON005Why),
      'DAEMON_006' => (l10n.diagDAEMON006Title, l10n.diagDAEMON006Why),
      'DAEMON_007' => (l10n.diagDAEMON007Title, l10n.diagDAEMON007Why),
      'DAEMON_008' => (l10n.diagDAEMON008Title, l10n.diagDAEMON008Why),
      'DAEMON_009' => (l10n.diagDAEMON009Title, l10n.diagDAEMON009Why),
      'DAEMON_010' => (l10n.diagDAEMON010Title, l10n.diagDAEMON010Why),
      'DAEMON_011' => (l10n.diagDAEMON011Title, l10n.diagDAEMON011Why),
      'DAEMON_012' => (l10n.diagDAEMON012Title, l10n.diagDAEMON012Why),
      'DAEMON_013' => (l10n.diagDAEMON013Title, l10n.diagDAEMON013Why),
      'DAEMON_014' => (l10n.diagDAEMON014Title, l10n.diagDAEMON014Why),
      'DOCTOR_001' => (l10n.diagDOCTOR001Title, l10n.diagDOCTOR001Why),
      'DOCTOR_002' => (l10n.diagDOCTOR002Title, l10n.diagDOCTOR002Why),
      'DOCTOR_003' => (l10n.diagDOCTOR003Title, l10n.diagDOCTOR003Why),
      'DOCTOR_004' => (l10n.diagDOCTOR004Title, l10n.diagDOCTOR004Why),
      'DOCTOR_005' => (l10n.diagDOCTOR005Title, l10n.diagDOCTOR005Why),
      'DOCTOR_006' => (l10n.diagDOCTOR006Title, l10n.diagDOCTOR006Why),
      'DOCTOR_007' => (l10n.diagDOCTOR007Title, l10n.diagDOCTOR007Why),
      'DOCTOR_008' => (l10n.diagDOCTOR008Title, l10n.diagDOCTOR008Why),
      'DOCTOR_009' => (l10n.diagDOCTOR009Title, l10n.diagDOCTOR009Why),
      'DOCTOR_010' => (l10n.diagDOCTOR010Title, l10n.diagDOCTOR010Why),
      'DOCTOR_011' => (l10n.diagDOCTOR011Title, l10n.diagDOCTOR011Why),
      'DOCTOR_012' => (l10n.diagDOCTOR012Title, l10n.diagDOCTOR012Why),
      'DOCTOR_013' => (l10n.diagDOCTOR013Title, l10n.diagDOCTOR013Why),
      'EDIT_001' => (l10n.diagEDIT001Title, l10n.diagEDIT001Why),
      'EDIT_002' => (l10n.diagEDIT002Title, l10n.diagEDIT002Why),
      'EDIT_003' => (l10n.diagEDIT003Title, l10n.diagEDIT003Why),
      'EDIT_005' => (l10n.diagEDIT005Title, l10n.diagEDIT005Why),
      'FINDINGS_001' => (l10n.diagFINDINGS001Title, l10n.diagFINDINGS001Why),
      'FINDINGS_002' => (l10n.diagFINDINGS002Title, l10n.diagFINDINGS002Why),
      'FINDINGS_003' => (l10n.diagFINDINGS003Title, l10n.diagFINDINGS003Why),
      'HOLD_004' => (l10n.diagHOLD004Title, l10n.diagHOLD004Why),
      'IPC_001' => (l10n.diagIPC001Title, l10n.diagIPC001Why),
      'IPC_002' => (l10n.diagIPC002Title, l10n.diagIPC002Why),
      'IPC_003' => (l10n.diagIPC003Title, l10n.diagIPC003Why),
      'IPC_004' => (l10n.diagIPC004Title, l10n.diagIPC004Why),
      'IPC_005' => (l10n.diagIPC005Title, l10n.diagIPC005Why),
      'IPC_006' => (l10n.diagIPC006Title, l10n.diagIPC006Why),
      'LLM_001' => (l10n.diagLLM001Title, l10n.diagLLM001Why),
      'LLM_002' => (l10n.diagLLM002Title, l10n.diagLLM002Why),
      'LLM_003' => (l10n.diagLLM003Title, l10n.diagLLM003Why),
      'LLM_004' => (l10n.diagLLM004Title, l10n.diagLLM004Why),
      'LLM_005' => (l10n.diagLLM005Title, l10n.diagLLM005Why),
      'LLM_006' => (l10n.diagLLM006Title, l10n.diagLLM006Why),
      'LLM_007' => (l10n.diagLLM007Title, l10n.diagLLM007Why),
      'LLM_008' => (l10n.diagLLM008Title, l10n.diagLLM008Why),
      'PROXY_001' => (l10n.diagPROXY001Title, l10n.diagPROXY001Why),
      'PROXY_002' => (l10n.diagPROXY002Title, l10n.diagPROXY002Why),
      'PROXY_003' => (l10n.diagPROXY003Title, l10n.diagPROXY003Why),
      'PROXY_005' => (l10n.diagPROXY005Title, l10n.diagPROXY005Why),
      'PROXY_007' => (l10n.diagPROXY007Title, l10n.diagPROXY007Why),
      'PROXY_008' => (l10n.diagPROXY008Title, l10n.diagPROXY008Why),
      'PROXY_009' => (l10n.diagPROXY009Title, l10n.diagPROXY009Why),
      'PROXY_010' => (l10n.diagPROXY010Title, l10n.diagPROXY010Why),
      'PROXY_011' => (l10n.diagPROXY011Title, l10n.diagPROXY011Why),
      'RECORDER_001' => (l10n.diagRECORDER001Title, l10n.diagRECORDER001Why),
      'RECORDER_002' => (l10n.diagRECORDER002Title, l10n.diagRECORDER002Why),
      'RECORDER_003' => (l10n.diagRECORDER003Title, l10n.diagRECORDER003Why),
      'RECORDER_004' => (l10n.diagRECORDER004Title, l10n.diagRECORDER004Why),
      'RULES_001' => (l10n.diagRULES001Title, l10n.diagRULES001Why),
      'RULES_002' => (l10n.diagRULES002Title, l10n.diagRULES002Why),
      'RULES_003' => (l10n.diagRULES003Title, l10n.diagRULES003Why),
      'RULES_005' => (l10n.diagRULES005Title, l10n.diagRULES005Why),
      'RULES_006' => (l10n.diagRULES006Title, l10n.diagRULES006Why),
      'RULES_007' => (l10n.diagRULES007Title, l10n.diagRULES007Why),
      'RULES_008' => (l10n.diagRULES008Title, l10n.diagRULES008Why),
      'RULES_009' => (l10n.diagRULES009Title, l10n.diagRULES009Why),
      'RULES_010' => (l10n.diagRULES010Title, l10n.diagRULES010Why),
      'RULES_011' => (l10n.diagRULES011Title, l10n.diagRULES011Why),
      'RULES_012' => (l10n.diagRULES012Title, l10n.diagRULES012Why),
      'SANDBOX_001' => (l10n.diagSANDBOX001Title, l10n.diagSANDBOX001Why),
      'SANDBOX_002' => (l10n.diagSANDBOX002Title, l10n.diagSANDBOX002Why),
      'SANDBOX_003' => (l10n.diagSANDBOX003Title, l10n.diagSANDBOX003Why),
      'SANDBOX_004' => (l10n.diagSANDBOX004Title, l10n.diagSANDBOX004Why),
      'SANDBOX_005' => (l10n.diagSANDBOX005Title, l10n.diagSANDBOX005Why),
      'SANDBOX_006' => (l10n.diagSANDBOX006Title, l10n.diagSANDBOX006Why),
      'SANDBOX_007' => (l10n.diagSANDBOX007Title, l10n.diagSANDBOX007Why),
      'SANDBOX_010' => (l10n.diagSANDBOX010Title, l10n.diagSANDBOX010Why),
      'SANDBOX_011' => (l10n.diagSANDBOX011Title, l10n.diagSANDBOX011Why),
      'SANDBOX_012' => (l10n.diagSANDBOX012Title, l10n.diagSANDBOX012Why),
      'SANDBOX_013' => (l10n.diagSANDBOX013Title, l10n.diagSANDBOX013Why),
      'SANDBOX_014' => (l10n.diagSANDBOX014Title, l10n.diagSANDBOX014Why),
      'SANDBOX_015' => (l10n.diagSANDBOX015Title, l10n.diagSANDBOX015Why),
      'SANDBOX_016' => (l10n.diagSANDBOX016Title, l10n.diagSANDBOX016Why),
      'SANDBOX_020' => (l10n.diagSANDBOX020Title, l10n.diagSANDBOX020Why),
      'SANDBOX_021' => (l10n.diagSANDBOX021Title, l10n.diagSANDBOX021Why),
      'SANDBOX_022' => (l10n.diagSANDBOX022Title, l10n.diagSANDBOX022Why),
      'SANDBOX_023' => (l10n.diagSANDBOX023Title, l10n.diagSANDBOX023Why),
      'SANDBOX_024' => (l10n.diagSANDBOX024Title, l10n.diagSANDBOX024Why),
      'SANDBOX_025' => (l10n.diagSANDBOX025Title, l10n.diagSANDBOX025Why),
      'SANDBOX_026' => (l10n.diagSANDBOX026Title, l10n.diagSANDBOX026Why),
      'SANDBOX_027' => (l10n.diagSANDBOX027Title, l10n.diagSANDBOX027Why),
      'SANDBOX_028' => (l10n.diagSANDBOX028Title, l10n.diagSANDBOX028Why),
      'SANDBOX_029' => (l10n.diagSANDBOX029Title, l10n.diagSANDBOX029Why),
      'TERM_001' => (l10n.diagTERM001Title, l10n.diagTERM001Why),
      'TERM_002' => (l10n.diagTERM002Title, l10n.diagTERM002Why),
      'TLS_001' => (l10n.diagTLS001Title, l10n.diagTLS001Why),
      'TLS_002' => (l10n.diagTLS002Title, l10n.diagTLS002Why),
      'TLS_003' => (l10n.diagTLS003Title, l10n.diagTLS003Why),
      'TLS_004' => (l10n.diagTLS004Title, l10n.diagTLS004Why),
      'TLS_005' => (l10n.diagTLS005Title, l10n.diagTLS005Why),
      'UI_002' => (l10n.diagUI002Title, l10n.diagUI002Why),
      _ => null,
    };
