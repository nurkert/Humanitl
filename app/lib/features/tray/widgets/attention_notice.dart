/// The one notice the shell shows about the desktop side of this program
/// (HUM-034).
///
/// It is a `Diagnostic` like every other non-green state: code, severity,
/// cause and a fix, in the same card the setup screen uses, so that a person
/// who has seen one has seen them all (`docs/UX.md` 4.4).
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/diagnostic_severity.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';

/// The notice.
class AttentionNoticeCard extends StatelessWidget {
  /// Creates the card for [diagnostic].
  const AttentionNoticeCard({
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
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          HDiagnosticCard(
            code: diagnostic.code,
            severityLabel: severityLabel(l10n, diagnostic.severity),
            color: severityColor(tokens, diagnostic.severity),
            title: title,
            why: why,
            detail: diagnostic.why.isEmpty ? null : diagnostic.why,
            fix: FixControl(fix: diagnostic.fix),
            docsUrl: diagnostic.docsUrl,
          ),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: const Key('attention-notice-dismiss'),
            variant: HButtonVariant.ghost,
            onPressed: onDismiss,
            child: Text(l10n.trayDismiss),
          ),
        ],
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
        DiagnosticCodes.decideRequestInvalid => (
          l10n.trayNoticeFindingsTitle,
          l10n.trayNoticeFindingsWhy,
        ),
        _ => (
          diagnostic.title.isEmpty ? diagnostic.code : diagnostic.title,
          diagnostic.why,
        ),
      };
}
