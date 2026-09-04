/// A card that shows one diagnostic: code, severity, title, cause, detail,
/// fix and documentation. Specified for `packages/ui` (HUM-019 Schritt 6);
/// it lives here until that package is touched again (handoff).
///
/// Like every `H*` widget it holds no user-visible string: every label comes
/// in already localised.
library;

import 'package:flutter/widgets.dart';

import 'ui.dart';

/// The card.
class HDiagnosticCard extends StatelessWidget {
  /// Creates a card.
  const HDiagnosticCard({
    required this.code,
    required this.severityLabel,
    required this.color,
    required this.title,
    required this.why,
    this.detail,
    this.fix,
    this.docsUrl,
    this.width = 560,
    super.key,
  });

  /// The registered code, for example `DAEMON_001`.
  final String code;

  /// The severity, already localised.
  final String severityLabel;

  /// The severity hue; never the blocked red.
  final Color color;

  /// The fixed part of the message.
  final String title;

  /// The cause, in the person's language.
  final String why;

  /// The technical detail, shown in monospace when present.
  final String? detail;

  /// The fix control, when there is one.
  final Widget? fix;

  /// Link to the documentation anchor, shown as text.
  final String? docsUrl;

  /// Width of the card.
  final double width;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final String? detail = this.detail;
    final Widget? fix = this.fix;
    final String? docsUrl = this.docsUrl;
    return Semantics(
      container: true,
      label: '$code $title',
      child: SizedBox(
        width: width,
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: tokens.colors.bg2,
            borderRadius: BorderRadius.circular(tokens.radii.card),
            border: Border.all(color: tokens.colors.lineStrong),
          ),
          child: ClipRRect(
            borderRadius: BorderRadius.circular(tokens.radii.card),
            // The rail stretches to the text; `IntrinsicHeight` gives the row
            // a height when the card sits in a scroll view without one.
            child: IntrinsicHeight(
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  SizedBox(
                    width: HSize.stateRail,
                    child: ColoredBox(color: color),
                  ),
                  Expanded(
                    child: Padding(
                      padding: EdgeInsets.all(tokens.spacing.x4),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisSize: MainAxisSize.min,
                        children: <Widget>[
                          // `Wrap` und nicht `Row`: bei doppelter
                          // Textskalierung passen Code und Schweregrad nicht
                          // mehr nebeneinander, und eine Zeile schnitte sie
                          // ab, statt umzubrechen (`docs/UX.md` 6).
                          Wrap(
                            spacing: tokens.spacing.x2,
                            runSpacing: tokens.spacing.x1,
                            children: <Widget>[
                              HBadge(text: code, color: color, mono: true),
                              HBadge(text: severityLabel, color: color),
                            ],
                          ),
                          SizedBox(height: tokens.spacing.x2),
                          Text(
                            title,
                            style: tokens.typography.ui16.semibold.tinted(
                              tokens.colors.fg0,
                            ),
                          ),
                          SizedBox(height: tokens.spacing.x2),
                          Text(
                            why,
                            style: tokens.typography.ui13.tinted(
                              tokens.colors.fg1,
                            ),
                          ),
                          if (detail != null && detail.isNotEmpty) ...<Widget>[
                            SizedBox(height: tokens.spacing.x3),
                            DecoratedBox(
                              decoration: BoxDecoration(
                                color: tokens.colors.bg1,
                                borderRadius: BorderRadius.circular(
                                  tokens.radii.control,
                                ),
                                border: Border.all(color: tokens.colors.line),
                              ),
                              child: Padding(
                                padding: EdgeInsets.symmetric(
                                  horizontal: tokens.spacing.x3,
                                  vertical: tokens.spacing.x2,
                                ),
                                child: Text(
                                  detail,
                                  style: tokens.typography.mono12.tinted(
                                    tokens.colors.fg1,
                                  ),
                                ),
                              ),
                            ),
                          ],
                          if (fix != null) ...<Widget>[
                            SizedBox(height: tokens.spacing.x3),
                            fix,
                          ],
                          if (docsUrl != null &&
                              docsUrl.isNotEmpty) ...<Widget>[
                            SizedBox(height: tokens.spacing.x3),
                            Text(
                              docsUrl,
                              // Der Akzent ist eine Fläche; ein Wort darauf
                              // nimmt seine Textvariante (`docs/UX.md` 6).
                              style: tokens.typography.mono12.tinted(
                                tokens.colors.accentText,
                              ),
                            ),
                          ],
                        ],
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
