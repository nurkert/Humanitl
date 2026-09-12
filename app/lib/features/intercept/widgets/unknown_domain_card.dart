/// Die Unbekannt-Karte des rechten Panes (HUM-031, HUM-094).
///
/// Der gestrichelte Rahmen sagt ohne ein Wort, dass hier etwas fehlt. Was
/// fehlt, ist ein Eintrag im Katalog — nicht eine Bewertung: „Not in the
/// catalog" heißt, dass dieser Bau nichts über den Dienst behauptet, nie, dass
/// er gefährlich sei. Die Karte zeigt deshalb nur Gemessenes: den Host, die
/// registrierbare Domäne des Daemons, den Verbreitungsrang und den Zähler
/// dieser Sitzung.
library;

import 'dart:ui' show PathMetric;

// `Flow` ist hier ein Domänentyp, nicht das Layout-Widget gleichen Namens.
import 'package:flutter/widgets.dart' hide Flow;

import '../../../core/domain/domain.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'domain_facts.dart';

/// Das Ziel, zu dem der Katalog nichts sagt.
class UnknownDomainCard extends StatelessWidget {
  /// Erzeugt die Karte für [flow].
  const UnknownDomainCard({
    required this.flow,
    required this.domain,
    super.key,
  });

  /// Die ausgewählte Anfrage.
  final Flow flow;

  /// Was der Daemon zum Ziel weiß; null, solange das Detail lädt.
  final DomainInfo? domain;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        CustomPaint(
          painter: DashedBorderPainter(
            color: tokens.colors.lineStrong,
            radius: tokens.radii.card,
          ),
          child: Padding(
            padding: EdgeInsets.all(tokens.spacing.x3),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                // Der gestrichelte Rahmen sagt schon, dass hier etwas fehlt;
                // ein Platzhalter-Zeichen daneben verspräche ein Bild, das
                // dieser Bau nie holt (ADR-006).
                Text(
                  l10n.interceptDomainNotInCatalog,
                  key: const Key('intercept-domain-unknown'),
                  style: tokens.typography.ui13.medium.tinted(
                    tokens.colors.fg1,
                  ),
                ),
                SizedBox(height: tokens.spacing.x2),
                DomainFacts(flow: flow, domain: domain),
              ],
            ),
          ),
        ),
        SizedBox(height: tokens.spacing.x3),
        // Der Knopf bleibt aus und sagt im Hover, warum: Ein toter Zustand
        // ohne Grund ist schlimmer als ein fehlender
        // (`backlog/CONVENTIONS.md` 4.13). Geholt wird ohnehin nie etwas von
        // selbst (ADR-006).
        HoverLabel(
          label: l10n.domainPreviewDisabled,
          child: HButton(
            key: const Key('intercept-domain-preview'),
            onPressed: null,
            child: Text(l10n.domainPreviewButton),
          ),
        ),
      ],
    );
  }
}

/// Ein gestricheltes, gerundetes Rechteck.
///
/// Der Rahmen ist die Aussage: Was hier steht, ist nicht aus dem Katalog.
class DashedBorderPainter extends CustomPainter {
  /// Zeichnet den Rahmen in [color] mit dem Radius [radius].
  DashedBorderPainter({required this.color, required this.radius});

  /// Die Farbe des Strichs.
  final Color color;

  /// Der Eckenradius.
  final double radius;

  static const double _dash = 4;
  static const double _gap = 3;

  @override
  void paint(Canvas canvas, Size size) {
    final Paint stroke = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = HSize.hairline
      ..color = color;
    final Path border = Path()
      ..addRRect(
        RRect.fromRectAndRadius(Offset.zero & size, Radius.circular(radius)),
      );
    for (final PathMetric metric in border.computeMetrics()) {
      double start = 0;
      while (start < metric.length) {
        final double end = start + _dash;
        canvas.drawPath(
          metric.extractPath(start, end.clamp(0, metric.length)),
          stroke,
        );
        start = end + _gap;
      }
    }
  }

  @override
  bool shouldRepaint(DashedBorderPainter oldDelegate) =>
      oldDelegate.color != color || oldDelegate.radius != radius;
}
