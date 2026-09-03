/// The right pane: what is known about the target domain.
///
/// Version 1 knows the host, the registrable domain under it and that the
/// catalog has nothing to say; HUM-031 fills the card and turns the two
/// buttons on.
library;

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'dart:ui' show PathMetric;

import 'package:flutter/widgets.dart' hide Flow;

import '../../../core/domain/domain.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../psl.dart';

/// The domain pane.
class DomainPanePlaceholder extends StatelessWidget {
  /// Creates the pane for [flow]; a null flow leaves it empty.
  const DomainPanePlaceholder({required this.flow, super.key});

  /// The selected flow, or null.
  final Flow? flow;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Flow? flow = this.flow;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: tokens.colors.bg1,
        border: Border(left: BorderSide(color: tokens.colors.line)),
      ),
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(
              l10n.interceptDomainTitle,
              style: tokens.typography.ui13.semibold.tinted(tokens.colors.fg0),
            ),
            if (flow != null) ...<Widget>[
              SizedBox(height: tokens.spacing.x3),
              Text(
                flow.host,
                key: const Key('intercept-domain-host'),
                style: tokens.typography.ui20.semibold.tinted(
                  tokens.colors.fg0,
                ),
              ),
              SizedBox(height: tokens.spacing.x1),
              Text(
                l10n.interceptDomainApex,
                style: tokens.typography.ui11.tinted(tokens.colors.fg2),
              ),
              Text(
                registrableDomain(
                  flow.host,
                  isIpLiteral: flow.authority.isIpLiteral,
                ),
                key: const Key('intercept-domain-apex'),
                style: tokens.typography.mono12.tinted(tokens.colors.fg1),
              ),
              SizedBox(height: tokens.spacing.x4),
              const _NotInCatalogCard(),
              SizedBox(height: tokens.spacing.x4),
              HoverLabel(
                label: l10n.interceptDomainSoon,
                child: HButton(
                  onPressed: null,
                  child: Text(l10n.interceptDomainRuleButton),
                ),
              ),
              SizedBox(height: tokens.spacing.x2),
              HoverLabel(
                label: l10n.interceptDomainSoon,
                child: HButton(
                  onPressed: null,
                  child: Text(l10n.interceptDomainCatalogButton),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// The dashed card that says the catalog has no entry yet.
class _NotInCatalogCard extends StatelessWidget {
  const _NotInCatalogCard();

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return CustomPaint(
      painter: _DashedBorderPainter(
        color: tokens.colors.line,
        radius: tokens.radii.card,
      ),
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Text(
              l10n.interceptDomainNotInCatalog,
              style: tokens.typography.ui12.medium.tinted(tokens.colors.fg1),
            ),
            SizedBox(height: tokens.spacing.x1),
            Text(
              l10n.interceptDomainFirstSeen,
              style: tokens.typography.ui12.tinted(tokens.colors.fg2),
            ),
          ],
        ),
      ),
    );
  }
}

/// A dashed rounded rectangle: the card is a placeholder, and the border says
/// so without a word.
class _DashedBorderPainter extends CustomPainter {
  _DashedBorderPainter({required this.color, required this.radius});

  final Color color;
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
  bool shouldRepaint(_DashedBorderPainter oldDelegate) =>
      oldDelegate.color != color || oldDelegate.radius != radius;
}
