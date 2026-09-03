/// The five glyphs of the icon rail, painted like `HGlyphIcon` paints the
/// state glyphs: Lucide shapes on a 24-unit box, no icon font. Candidates for
/// `HGlyph` in `packages/ui` (handoff of HUM-019).
library;

import 'package:flutter/widgets.dart';

import 'ui.dart';

/// A rail glyph.
enum ShellGlyph {
  /// Lucide `hourglass`: the queue of held requests.
  intercept,

  /// Lucide `history`: the recorded flows.
  history,

  /// Lucide `list-checks`: the rule list.
  rules,

  /// Lucide `box`: the sandbox.
  sandbox,

  /// Lucide `scroll-text`: the audit log.
  audit,
}

/// Draws one [ShellGlyph] at [size] in [color].
class ShellGlyphIcon extends StatelessWidget {
  /// Draws [glyph].
  const ShellGlyphIcon(this.glyph, {this.size = 18, this.color, super.key});

  /// The shape.
  final ShellGlyph glyph;

  /// Edge length of the square the glyph is drawn into.
  final double size;

  /// Stroke colour; the secondary text colour when null.
  final Color? color;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    if (glyph == ShellGlyph.intercept) {
      return HGlyphIcon(HGlyph.hourglass, size: size, color: color);
    }
    return ExcludeSemantics(
      child: CustomPaint(
        size: Size.square(size),
        painter: _ShellGlyphPainter(
          glyph: glyph,
          color: color ?? tokens.colors.fg1,
        ),
      ),
    );
  }
}

class _ShellGlyphPainter extends CustomPainter {
  _ShellGlyphPainter({required this.glyph, required this.color});

  final ShellGlyph glyph;
  final Color color;

  static const double _viewBox = 24;

  @override
  void paint(Canvas canvas, Size size) {
    final double scale = size.shortestSide / _viewBox;
    canvas.save();
    canvas.scale(scale, scale);
    final Paint stroke = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round
      ..color = color;
    switch (glyph) {
      case ShellGlyph.intercept:
        break;
      case ShellGlyph.history:
        canvas
          ..drawArc(const Rect.fromLTWH(3, 3, 18, 18), -2.6, 5.4, false, stroke)
          ..drawPath(
            _polyline(const <Offset>[Offset(3, 3), Offset(3, 8), Offset(8, 8)]),
            stroke,
          )
          ..drawPath(
            _polyline(const <Offset>[
              Offset(12, 7),
              Offset(12, 12),
              Offset(16, 14),
            ]),
            stroke,
          );
      case ShellGlyph.rules:
        canvas
          ..drawPath(
            _polyline(const <Offset>[Offset(3, 6), Offset(5, 8), Offset(9, 4)]),
            stroke,
          )
          ..drawPath(
            _polyline(const <Offset>[
              Offset(3, 16),
              Offset(5, 18),
              Offset(9, 14),
            ]),
            stroke,
          )
          ..drawLine(const Offset(13, 6), const Offset(21, 6), stroke)
          ..drawLine(const Offset(13, 12), const Offset(21, 12), stroke)
          ..drawLine(const Offset(13, 18), const Offset(21, 18), stroke);
      case ShellGlyph.sandbox:
        canvas
          ..drawPath(
            _polyline(const <Offset>[
              Offset(12, 2.5),
              Offset(20.5, 7),
              Offset(20.5, 17),
              Offset(12, 21.5),
              Offset(3.5, 17),
              Offset(3.5, 7),
            ], close: true),
            stroke,
          )
          ..drawPath(
            _polyline(const <Offset>[
              Offset(3.5, 7),
              Offset(12, 12),
              Offset(20.5, 7),
            ]),
            stroke,
          )
          ..drawLine(const Offset(12, 12), const Offset(12, 21.5), stroke);
      case ShellGlyph.audit:
        canvas
          ..drawPath(
            _polyline(const <Offset>[
              Offset(8, 3),
              Offset(19, 3),
              Offset(19, 21),
              Offset(8, 21),
            ]),
            stroke,
          )
          ..drawPath(
            _polyline(const <Offset>[
              Offset(8, 3),
              Offset(5, 3),
              Offset(5, 8),
              Offset(8, 8),
            ]),
            stroke,
          )
          ..drawPath(
            _polyline(const <Offset>[
              Offset(8, 21),
              Offset(5, 21),
              Offset(5, 16),
              Offset(8, 16),
            ]),
            stroke,
          )
          ..drawLine(const Offset(11, 9), const Offset(16, 9), stroke)
          ..drawLine(const Offset(11, 13), const Offset(16, 13), stroke);
    }
    canvas.restore();
  }

  static Path _polyline(List<Offset> points, {bool close = false}) {
    final Path path = Path()..moveTo(points.first.dx, points.first.dy);
    for (final Offset point in points.skip(1)) {
      path.lineTo(point.dx, point.dy);
    }
    if (close) {
      path.close();
    }
    return path;
  }

  @override
  bool shouldRepaint(_ShellGlyphPainter oldDelegate) =>
      oldDelegate.glyph != glyph || oldDelegate.color != color;
}
