/// The diagnostics this feature raises on its own (HUM-034).
///
/// All of them are facts about the world outside the program, not failures
/// inside it, and all of them carry cause and fix like every other non-green
/// state (`docs/UX.md` 4.4). The codes themselves live in
/// `core/domain/diagnostic_codes.dart`, because every code the app names is a
/// registered one (`backlog/CONVENTIONS.md` 4.6).
library;

import '../../core/domain/domain.dart';

/// Codes and factories of the tray.
abstract final class TrayDiagnostics {
  /// Where the fix of [DiagnosticCodes.noTray] leads.
  ///
  /// The extension is the fix, so the link goes to the extension and not to a
  /// page of ours that would only name it.
  static const String extensionUrl =
      'https://extensions.gnome.org/extension/615/appindicator-support/';

  /// `UI_002`: this desktop has no tray. Information, not a failure: the
  /// count still stands in the window title, which every desktop shows.
  static Diagnostic trayUnavailable(String why) => Diagnostic(
    code: DiagnosticCodes.noTray,
    severity: Severity.info,
    why: why,
    fix: const FixAction.openUrl(url: extensionUrl),
    docsUrl: extensionUrl,
  );

  /// `IPC_003`: a notification button was pressed for a request that had
  /// already left the queue.
  ///
  /// The registered code of "flow no longer held", raised by the client
  /// rather than the daemon because the client never sent the decision: what
  /// the message named is gone, and deciding whatever took its place would be
  /// the one thing `docs/UX.md` 4.9 forbids.
  static Diagnostic decidedAlready(FlowId flowId) => Diagnostic(
    code: DiagnosticCodes.flowNotHeld,
    severity: Severity.info,
    why: 'flow ${flowId.value} was no longer held',
  );

  /// `IPC_004`: `Allow` was pressed on a message for a request that carries a
  /// finding.
  ///
  /// Analysis and hold are two events and the second can follow the first, so
  /// a message that offered `Allow` can be standing on the screen when the
  /// finding arrives. Sending such a request asks for the held confirmation
  /// and a sentence naming what goes where (`docs/UX.md` 4.7), and a
  /// notification button is neither; the decision is refused here rather than
  /// sent, and the window comes forward instead.
  static Diagnostic findingsNeedTheWindow(FlowId flowId, int findings) =>
      Diagnostic(
        code: DiagnosticCodes.decideRequestInvalid,
        severity: Severity.info,
        why:
            'flow ${flowId.value} carries $findings '
            '${findings == 1 ? 'finding' : 'findings'}; '
            'allowing it needs the confirmation in the window',
      );
}
