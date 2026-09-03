/// The 40 px header: wordmark, section title, intercept pill, hold count,
/// isolation ring placeholder and the palette button.
library;

import 'dart:math' as math;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../section.dart';

/// The header.
class HeaderBar extends StatelessWidget {
  /// Creates the header for [section].
  const HeaderBar({
    required this.section,
    required this.onPalette,
    this.heldCount = 0,
    super.key,
  });

  /// The shown section; its title sits next to the wordmark.
  final Section section;

  /// Opens the command palette.
  final VoidCallback onPalette;

  /// Number of held requests, shown as a badge. HUM-020 supplies it.
  final int heldCount;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return SizedBox(
      height: tokens.sizes.headerBar,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: tokens.colors.bg1,
          border: Border(bottom: BorderSide(color: tokens.colors.line)),
        ),
        child: Padding(
          padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
          child: Row(
            children: <Widget>[
              const Wordmark(),
              SizedBox(width: tokens.spacing.x4),
              Text(
                section.label(l10n),
                key: const Key('header-section-title'),
                style: tokens.typography.ui13.medium.tinted(tokens.colors.fg1),
              ),
              const Spacer(),
              HBadge(text: l10n.shellInterceptOn, color: tokens.state.allowed),
              SizedBox(width: tokens.spacing.x2),
              HBadge(
                text: l10n.shellHeldCount(heldCount),
                color: heldCount > 0 ? tokens.state.held : tokens.colors.fg1,
              ),
              SizedBox(width: tokens.spacing.x3),
              IsolationRingPlaceholder(
                semanticsLabel: l10n.shellIsolationUnknown,
              ),
              SizedBox(width: tokens.spacing.x3),
              HButton(
                key: const Key('header-palette-button'),
                variant: HButtonVariant.ghost,
                onPressed: onPalette,
                semanticsLabel: l10n.shellPaletteTitle,
                child: Text(l10n.shellPaletteHint),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The accent mark and the product name.
class Wordmark extends StatelessWidget {
  /// Creates the wordmark.
  const Wordmark({this.markSize = 16, this.style, super.key});

  /// Edge length of the accent mark.
  final double markSize;

  /// Text style of the name; 13/600 when null.
  final TextStyle? style;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        SizedBox.square(
          dimension: markSize,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: tokens.colors.accent,
              borderRadius: BorderRadius.circular(tokens.radii.control),
            ),
            child: Center(
              child: SizedBox(
                width: markSize * 0.375,
                height: markSize * 0.125,
                child: ColoredBox(color: tokens.colors.onAccent),
              ),
            ),
          ),
        ),
        SizedBox(width: tokens.spacing.x2),
        Text(
          context.l10n.appTitle,
          style:
              style ??
              tokens.typography.ui13.semibold.tinted(tokens.colors.fg0),
        ),
      ],
    );
  }
}

/// The 20 px isolation ring in grey: three segments for the three
/// guarantees, none of them checked yet (HUM-041 colours them).
class IsolationRingPlaceholder extends StatelessWidget {
  /// Creates the placeholder ring.
  const IsolationRingPlaceholder({
    required this.semanticsLabel,
    this.size = 20,
    super.key,
  });

  /// Screen-reader label, localised.
  final String semanticsLabel;

  /// Diameter of the ring.
  final double size;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Semantics(
      label: semanticsLabel,
      child: CustomPaint(
        size: Size.square(size),
        painter: _RingPainter(color: tokens.colors.fg2),
      ),
    );
  }
}

class _RingPainter extends CustomPainter {
  _RingPainter({required this.color});

  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final Paint paint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2
      ..strokeCap = StrokeCap.butt
      ..color = color;
    final Rect rect = (Offset.zero & size).deflate(1);
    const double gap = 0.35;
    final double sweep = (2 * math.pi - 3 * gap) / 3;
    for (int i = 0; i < 3; i++) {
      final double start = -math.pi / 2 + i * (sweep + gap);
      canvas.drawArc(rect, start, sweep, false, paint);
    }
  }

  @override
  bool shouldRepaint(_RingPainter oldDelegate) => oldDelegate.color != color;
}
