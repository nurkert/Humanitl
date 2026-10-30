/// The one notice the shell shows about the desktop side of this program
/// (HUM-034).
///
/// It is a `Diagnostic` like every other non-green state: code, severity,
/// cause and a fix (`docs/UX.md` 4.4). Since HUM-302 it is a card that floats
/// over the top right of the window (`HNoticeLayer`), no longer a band under
/// the header: a band pushed every pane down and made a running terminal
/// wrap anew whenever it came or went. The name stays from the band of
/// HUM-276. Its close affordance sits inside the card, top right, which puts
/// it in the reading order before the fix below the text. Only a click or
/// `Space` on it closes the notice; `Esc` does not, because `Esc` belongs to
/// whatever the person is doing below it.
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/diagnostic_severity.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/h_notice_layer.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../tray_diagnostics.dart';

/// The notice card.
class AttentionNoticeBanner extends StatelessWidget {
  /// Creates the notice for [diagnostic].
  const AttentionNoticeBanner({
    required this.diagnostic,
    required this.onDismiss,
    super.key,
  });

  /// What happened.
  final Diagnostic diagnostic;

  /// Closes the notice. It does not come back for the same cause.
  final VoidCallback onDismiss;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final (String title, String why) = _text(l10n, diagnostic);
    // Der Satz des Daemons nur dann als Detail, wenn er nicht schon im
    // Grund-Platz steht (`docs/UX.md` 4.4).
    final String? detail = diagnostic.why.isEmpty || why == diagnostic.why
        ? null
        : diagnostic.why;
    return HDiagnosticCard(
      key: const Key('attention-notice'),
      code: diagnostic.code,
      severityLabel: severityLabel(l10n, diagnostic.severity),
      color: severityColor(tokens, diagnostic.severity),
      title: title,
      why: why,
      detail: detail,
      detailMaxLines: 2,
      fix: diagnostic.fix == null ? null : FixControl(fix: diagnostic.fix),
      width: double.infinity,
      dense: true,
      dismiss: HNoticeDismiss(
        label: l10n.trayDismiss,
        buttonKey: const Key('attention-notice-dismiss'),
        onPressed: onDismiss,
      ),
    );
  }

  /// Title and cause in the person's language.
  ///
  /// The same shape the setup screen uses: localised for the codes this
  /// client raises itself, the sender's own sentence for anything else.
  static (String, String) _text(AppLocalizations l10n, Diagnostic diagnostic) =>
      switch (diagnostic.code) {
        DiagnosticCodes.noTray => (
          l10n.trayNoticeNoTrayTitle,
          l10n.trayNoticeNoTrayWhy,
        ),
        DiagnosticCodes.flowNotHeld => (
          l10n.trayNoticeDecidedTitle,
          l10n.trayNoticeDecidedWhy,
        ),
        // Derselbe Code, ein anderer Grund: der WebSocket-Aufbau (HUM-058).
        DiagnosticCodes.decideRequestInvalid
            when diagnostic.title == TrayDiagnostics.connectionTitle =>
          (l10n.trayNoticeConnectionTitle, l10n.trayNoticeConnectionWhy),
        DiagnosticCodes.decideRequestInvalid
            when diagnostic.title == TrayDiagnostics.detailTitle =>
          (l10n.trayNoticeDetailTitle, l10n.trayNoticeDetailWhy),
        DiagnosticCodes.decideRequestInvalid => (
          l10n.trayNoticeFindingsTitle,
          l10n.trayNoticeFindingsWhy,
        ),
        // Jeder andere Code: Überschrift aus ARB (HUM-052), im Grund-Platz der
        // gemessene Satz des Daemons (`docs/UX.md` 4.4, CONVENTIONS 4.36).
        _ => _titleAndCause(DiagnosticL10n.resolve(diagnostic, l10n)),
      };

  static (String, String) _titleAndCause(DiagnosticText text) =>
      (text.title, text.cause);
}
