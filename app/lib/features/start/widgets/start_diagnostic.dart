/// A finding of the daemon in the start window, as a card the width of the
/// page (HUM-537): under the project list, on the first-start screen and
/// above the steps of the guided agent setup (GitHub #111).
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/diagnostic_severity.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/ui.dart';
import '../../../core/ui/untranslated_wording.dart';
import '../../../l10n/l10n.dart';

/// A finding of the daemon as a card the width of the page.
class StartDiagnostic extends StatelessWidget {
  /// Creates the card.
  const StartDiagnostic({required this.diagnostic, super.key});

  /// What the daemon said.
  final Diagnostic diagnostic;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final DiagnosticText text = DiagnosticL10n.resolve(diagnostic, l10n);
    return HDiagnosticCard(
      key: Key('start-diagnostic-${diagnostic.code}'),
      code: diagnostic.code,
      severityLabel: severityLabel(l10n, diagnostic.severity),
      color: severityColor(tokens, diagnostic.severity),
      title: text.title,
      why: text.cause,
      original: UntranslatedWording.forDiagnostic(diagnostic, l10n),
      docsUrl: diagnostic.docsUrl,
      width: double.infinity,
    );
  }
}
