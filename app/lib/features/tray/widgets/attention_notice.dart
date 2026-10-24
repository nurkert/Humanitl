/// The one notice the shell shows about the desktop side of this program
/// (HUM-034).
///
/// It is a `Diagnostic` like every other non-green state: code, severity,
/// cause and a fix (`docs/UX.md` 4.4). Since HUM-276 it is a banner and no
/// longer a card: it runs the full width of the window directly under the
/// header, above the rail and every pane, so it never stands in the queue
/// column and looks the same with the rail collapsed or expanded. Its
/// "Dismiss" button sits at the top of the right end, which puts it in the
/// reading order right after the header and before the fix below the text.
/// Only a click or `Space` on it closes the banner; `Esc` does not, because
/// `Esc` belongs to whatever the person is doing below it.
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/diagnostic_severity.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../tray_diagnostics.dart';

/// The banner.
class AttentionNoticeBanner extends StatelessWidget {
  /// Creates the banner for [diagnostic].
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
    final Color color = severityColor(tokens, diagnostic.severity);
    // Der Satz des Daemons nur dann als Detail, wenn er nicht schon im
    // Grund-Platz steht (`docs/UX.md` 4.4).
    final String? detail = diagnostic.why.isEmpty || why == diagnostic.why
        ? null
        : diagnostic.why;
    return Semantics(
      key: const Key('attention-notice'),
      container: true,
      label: '${diagnostic.code} $title',
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: HColorDerivation.tint(color),
          // Die Linie gehört dem oberen von zwei Bereichen (`docs/UX.md`
          // 3.7): Das Banner zieht sie unter sich, die Panes darunter
          // beginnen ohne eigene.
          border: Border(bottom: BorderSide(color: tokens.colors.line)),
        ),
        child: ConstrainedBox(
          // Eine Mindesthöhe, keine Höhe: Bei doppelter Textskalierung wächst
          // das Banner, statt den Rest still abzuschneiden (`docs/UX.md` 6).
          constraints: BoxConstraints(minHeight: tokens.sizes.row),
          child: Padding(
            padding: EdgeInsets.symmetric(
              horizontal: tokens.spacing.x3,
              vertical: tokens.spacing.x2,
            ),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                HBadge(text: diagnostic.code, color: color, mono: true),
                SizedBox(width: tokens.spacing.x2),
                HBadge(
                  text: severityLabel(l10n, diagnostic.severity),
                  color: color,
                ),
                SizedBox(width: tokens.spacing.x3),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      Text(
                        title,
                        key: const Key('attention-notice-title'),
                        style: tokens.typography.ui13.medium.tinted(
                          tokens.colors.fg0,
                        ),
                      ),
                      Text(
                        why,
                        key: const Key('attention-notice-why'),
                        style: tokens.typography.ui12.tinted(tokens.colors.fg0),
                      ),
                      if (detail != null)
                        Text(
                          detail,
                          key: const Key('attention-notice-detail'),
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: tokens.typography.mono12.tinted(
                            tokens.colors.fg0,
                          ),
                        ),
                      if (diagnostic.fix != null) ...<Widget>[
                        SizedBox(height: tokens.spacing.x2),
                        FixControl(fix: diagnostic.fix),
                      ],
                    ],
                  ),
                ),
                SizedBox(width: tokens.spacing.x3),
                HButton(
                  key: const Key('attention-notice-dismiss'),
                  variant: HButtonVariant.ghost,
                  onPressed: onDismiss,
                  child: Text(l10n.trayDismiss),
                ),
              ],
            ),
          ),
        ),
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
