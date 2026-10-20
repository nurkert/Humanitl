/// The findings the daemon attaches to the detail of one flow, one compact
/// line each (HUM-057, row 9 of the state table in HUM-058).
///
/// Der Daemon legt sie in `FlowDetail.diagnostics`, heute für einen Fluss,
/// den er am Cap geblockt hat: `LIMIT_001` mit dem Vorschlag einer Regel
/// `stream: true`. Die Oberfläche baut weder den Befund noch die Regel selbst
/// (ADR-018); sie zeigt, was kommt, mit Code, übersetztem Titel und dem Fix.
///
/// Kompakt heißt eine Zeile: Code, Titel, Fix. Der Satz des Daemons steht im
/// Semantik-Etikett der Zeile, damit ein Screenreader den Grund hört, den die
/// Zeile aus Platzgründen weglässt.
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../history_filter_bar.dart';

/// The compact diagnostics of one flow; draws nothing without any.
class HistoryDiagnostics extends StatelessWidget {
  /// Creates the list for [diagnostics].
  const HistoryDiagnostics({required this.diagnostics, super.key});

  /// What the daemon attached to the detail, in its order.
  final List<Diagnostic> diagnostics;

  @override
  Widget build(BuildContext context) {
    if (diagnostics.isEmpty) {
      return const SizedBox.shrink();
    }
    final HTokens tokens = HTheme.of(context);
    return Padding(
      padding: EdgeInsets.symmetric(
        horizontal: tokens.spacing.x3,
        vertical: tokens.spacing.x2,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          for (final (int index, Diagnostic diagnostic) in diagnostics.indexed)
            _CompactDiagnostic(
              key: ValueKey<String>('history-diagnostic-$index'),
              index: index,
              diagnostic: diagnostic,
            ),
        ],
      ),
    );
  }
}

/// One line: code, title, fix.
class _CompactDiagnostic extends StatelessWidget {
  const _CompactDiagnostic({
    required this.index,
    required this.diagnostic,
    super.key,
  });

  final int index;
  final Diagnostic diagnostic;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final DiagnosticText text = DiagnosticL10n.resolve(diagnostic, l10n);
    final Color color = historySeverityColor(tokens, diagnostic.severity);
    return Semantics(
      container: true,
      label: '${diagnostic.code} ${text.title}. ${text.cause}',
      // `Wrap` und nicht `Row`: Bei doppelter Textskalierung passen Code,
      // Titel und Fix nicht nebeneinander, und eine Zeile schnitte sie ab
      // (`docs/UX.md` 6).
      child: Wrap(
        spacing: tokens.spacing.x2,
        runSpacing: tokens.spacing.x1,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: <Widget>[
          HBadge(text: diagnostic.code, color: color, mono: true),
          Text(
            text.title,
            style: tokens.typography.ui13.tinted(tokens.colors.fg0),
          ),
          FixControl(
            fix: diagnostic.fix,
            copyKey: ValueKey<String>('history-diagnostic-fix-$index'),
          ),
        ],
      ),
    );
  }
}
