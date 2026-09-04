import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/flow_state.dart';
import '../tokens/tokens.dart';

/// Draws one [HGlyph] at [size], in [color].
///
/// The shapes are the Lucide icons named in HUM-008, painted instead of loaded
/// from an icon font. `packages/ui` has no component library yet; painting the
/// ten glyphs it needs keeps the package free of one and makes the later swap a
/// change in this file only.
class HGlyphIcon extends StatelessWidget {
  /// Draws [glyph].
  const HGlyphIcon(
    this.glyph, {
    this.size = 16,
    this.color,
    this.accentColor,
    this.semanticsLabel,
    super.key,
  });

  /// The shape to draw.
  final HGlyph glyph;

  /// Edge length of the square the glyph is drawn into.
  final double size;

  /// Stroke colour; the secondary text colour of the theme when null.
  final Color? color;

  /// Colour of the pencil dot of [HGlyph.arrowUpRightPencil]; the accent when
  /// null.
  final Color? accentColor;

  /// Screen-reader label. Without one the glyph is decorative.
  final String? semanticsLabel;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Widget painted = CustomPaint(
      size: Size.square(size),
      painter: _HGlyphPainter(
        glyph: glyph,
        color: color ?? tokens.colors.fg1,
        accentColor: accentColor ?? tokens.colors.accent,
      ),
      isComplex: false,
    );
    if (semanticsLabel == null) {
      return ExcludeSemantics(child: painted);
    }
    return Semantics(label: semanticsLabel, child: painted);
  }
}

class _HGlyphPainter extends CustomPainter {
  _HGlyphPainter({
    required this.glyph,
    required this.color,
    required this.accentColor,
  });

  final HGlyph glyph;
  final Color color;
  final Color accentColor;

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
    final Paint fill = Paint()
      ..style = PaintingStyle.fill
      ..color = color;
    switch (glyph) {
      case HGlyph.hourglass:
        canvas
          ..drawLine(const Offset(5, 3), const Offset(19, 3), stroke)
          ..drawLine(const Offset(5, 21), const Offset(19, 21), stroke)
          ..drawPath(
            _polyline(const <Offset>[
              Offset(7, 3),
              Offset(7, 7),
              Offset(12, 12),
              Offset(17, 7),
              Offset(17, 3),
            ]),
            stroke,
          )
          ..drawPath(
            _polyline(const <Offset>[
              Offset(7, 21),
              Offset(7, 17),
              Offset(12, 12),
              Offset(17, 17),
              Offset(17, 21),
            ]),
            stroke,
          );
      case HGlyph.arrowUpRight:
      case HGlyph.arrowUpRightPencil:
        canvas
          ..drawPath(
            _polyline(const <Offset>[
              Offset(7, 7),
              Offset(17, 7),
              Offset(17, 17),
            ]),
            stroke,
          )
          ..drawLine(const Offset(7, 17), const Offset(17, 7), stroke);
        if (glyph == HGlyph.arrowUpRightPencil) {
          canvas.drawCircle(
            const Offset(6, 19),
            3,
            Paint()..color = accentColor,
          );
        }
      case HGlyph.shieldX:
        canvas
          ..drawPath(
            _polyline(const <Offset>[
              Offset(12, 2),
              Offset(20, 5),
              Offset(20, 12),
              Offset(12, 22),
              Offset(4, 12),
              Offset(4, 5),
            ], close: true),
            stroke,
          )
          ..drawLine(const Offset(9.5, 9.5), const Offset(14.5, 14.5), stroke)
          ..drawLine(const Offset(14.5, 9.5), const Offset(9.5, 14.5), stroke);
      case HGlyph.clockX:
        canvas
          ..drawCircle(const Offset(11, 11), 8, stroke)
          ..drawPath(
            _polyline(const <Offset>[
              Offset(11, 6),
              Offset(11, 11),
              Offset(14, 13),
            ]),
            stroke,
          )
          ..drawLine(const Offset(16.5, 16.5), const Offset(21.5, 21.5), stroke)
          ..drawLine(
            const Offset(21.5, 16.5),
            const Offset(16.5, 21.5),
            stroke,
          );
      case HGlyph.bolt:
        canvas.drawPath(
          _polyline(const <Offset>[
            Offset(13, 2),
            Offset(4, 14),
            Offset(11, 14),
            Offset(11, 22),
            Offset(20, 10),
            Offset(13, 10),
          ], close: true),
          stroke,
        );
      case HGlyph.chevronsRight:
        canvas
          ..drawPath(
            _polyline(const <Offset>[
              Offset(6, 7),
              Offset(11, 12),
              Offset(6, 17),
            ]),
            stroke,
          )
          ..drawPath(
            _polyline(const <Offset>[
              Offset(13, 7),
              Offset(18, 12),
              Offset(13, 17),
            ]),
            stroke,
          );
      case HGlyph.triangleAlert:
        canvas
          ..drawPath(
            _polyline(const <Offset>[
              Offset(12, 3),
              Offset(22, 20),
              Offset(2, 20),
            ], close: true),
            stroke,
          )
          ..drawLine(const Offset(12, 9), const Offset(12, 14), stroke)
          ..drawLine(const Offset(12, 17), const Offset(12, 17.01), stroke);
      case HGlyph.chevronRight:
        canvas.drawPath(
          _polyline(const <Offset>[
            Offset(9, 6),
            Offset(15, 12),
            Offset(9, 18),
          ]),
          stroke,
        );
      case HGlyph.close:
        canvas
          ..drawLine(const Offset(6, 6), const Offset(18, 18), stroke)
          ..drawLine(const Offset(18, 6), const Offset(6, 18), stroke);
      case HGlyph.grip:
        for (final double x in const <double>[9, 15]) {
          for (final double y in const <double>[5, 12, 19]) {
            canvas.drawCircle(Offset(x, y), 1.4, fill);
          }
        }
      case HGlyph.trash:
        canvas
          ..drawLine(const Offset(3, 6), const Offset(21, 6), stroke)
          ..drawPath(
            _polyline(const <Offset>[
              Offset(5, 6),
              Offset(5, 20),
              Offset(19, 20),
              Offset(19, 6),
            ]),
            stroke,
          )
          ..drawPath(
            _polyline(const <Offset>[
              Offset(9, 6),
              Offset(9, 3),
              Offset(15, 3),
              Offset(15, 6),
            ]),
            stroke,
          );
      case HGlyph.plus:
        canvas
          ..drawLine(const Offset(12, 5), const Offset(12, 19), stroke)
          ..drawLine(const Offset(5, 12), const Offset(19, 12), stroke);
      case HGlyph.lock:
        canvas
          ..drawRRect(
            RRect.fromRectAndRadius(
              const Rect.fromLTWH(4, 11, 16, 10),
              const Radius.circular(2),
            ),
            stroke,
          )
          ..drawPath(
            _polyline(const <Offset>[
              Offset(8, 11),
              Offset(8, 7),
              Offset(12, 4),
              Offset(16, 7),
              Offset(16, 11),
            ]),
            stroke,
          );
      case HGlyph.redactBar:
        canvas
          ..drawLine(const Offset(4, 6), const Offset(20, 6), stroke)
          ..drawRect(const Rect.fromLTWH(4, 10, 16, 5), fill)
          ..drawLine(const Offset(4, 19), const Offset(14, 19), stroke);
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
  bool shouldRepaint(_HGlyphPainter oldDelegate) =>
      oldDelegate.glyph != glyph ||
      oldDelegate.color != color ||
      oldDelegate.accentColor != accentColor;
}
