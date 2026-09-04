/// The setup screen: what shows instead of an empty queue when the daemon is
/// missing, incompatible or rejects the token. A placeholder for the
/// checklist of HUM-044; today it renders the diagnostic and offers
/// "Reconnect".
library;

import 'package:flutter/widgets.dart';

import '../../core/domain/domain.dart';
import '../../core/ui/h_diagnostic_card.dart';
import '../../core/ui/fix_control.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';

/// The setup screen.
class SetupScreen extends StatelessWidget {
  /// Creates the screen for [diagnostic].
  const SetupScreen({
    required this.diagnostic,
    required this.onRetry,
    super.key,
  });

  /// Why the shell cannot show.
  final Diagnostic diagnostic;

  /// Tries the connection again.
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final (String title, String why) = localizedText(l10n, diagnostic);
    return ColoredBox(
      color: tokens.colors.bg0,
      child: Center(
        child: SingleChildScrollView(
          padding: EdgeInsets.all(tokens.spacing.x6),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                l10n.setupTitle,
                style: tokens.typography.ui20.semibold.tinted(
                  tokens.colors.fg0,
                ),
              ),
              SizedBox(height: tokens.spacing.x4),
              HDiagnosticCard(
                code: diagnostic.code,
                severityLabel: severityLabel(l10n, diagnostic.severity),
                color: severityColor(tokens, diagnostic.severity),
                title: title,
                why: why,
                detail: diagnostic.why,
                fix: FixControl(fix: diagnostic.fix),
                docsUrl: diagnostic.docsUrl,
              ),
              SizedBox(height: tokens.spacing.x3),
              SizedBox(
                width: 560,
                child: Text(
                  l10n.setupHint,
                  style: tokens.typography.ui13.tinted(tokens.colors.fg2),
                ),
              ),
              SizedBox(height: tokens.spacing.x4),
              HButton(
                key: const Key('setup-retry'),
                variant: HButtonVariant.primary,
                size: HButtonSize.md,
                autofocus: true,
                onPressed: onRetry,
                child: Text(l10n.setupRetry),
              ),
            ],
          ),
        ),
      ),
    );
  }

  /// Title and cause in the person's language: localised for the codes the
  /// client raises itself, the daemon's own text for everything else.
  static (String, String) localizedText(
    AppLocalizations l10n,
    Diagnostic diagnostic,
  ) => switch (diagnostic.code) {
    DiagnosticCodes.daemonUnreachable => (
      l10n.setupDaemonMissingTitle,
      l10n.setupDaemonMissingWhy,
    ),
    DiagnosticCodes.protoIncompatible => (
      l10n.setupVersionMismatchTitle,
      l10n.setupVersionMismatchWhy,
    ),
    DiagnosticCodes.tokenInvalid => (
      l10n.setupTokenInvalidTitle,
      l10n.setupTokenInvalidWhy,
    ),
    _ => (
      diagnostic.title.isEmpty ? diagnostic.code : diagnostic.title,
      diagnostic.why,
    ),
  };

  /// The severity's label.
  static String severityLabel(AppLocalizations l10n, Severity severity) =>
      switch (severity) {
        Severity.info => l10n.diagSeverityInfo,
        Severity.warning => l10n.diagSeverityWarning,
        Severity.error => l10n.diagSeverityError,
        Severity.blocking => l10n.diagSeverityBlocking,
      };

  /// The severity's hue. Never the blocked red: red means blocked.
  static Color severityColor(HTokens tokens, Severity severity) =>
      switch (severity) {
        Severity.info => tokens.colors.accent,
        Severity.warning => tokens.state.held,
        Severity.error || Severity.blocking => tokens.state.error,
      };
}
