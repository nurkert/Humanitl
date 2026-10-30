import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';

/// A one pixel separator. The design uses hairlines where others use shadows.
///
/// Die Linie nimmt im Layout [HSize.hairline] ein, einen logischen Pixel,
/// und zeichnet ganze physische Pixel ([RenderHairline]). Bei 100 % auf
/// einem Schirm ohne Skalierung ist das dasselbe; bei einer Skalierung der
/// Anwendung von 125 % oder 175 % (HUM-356) wäre ein logischer Pixel 1,25
/// oder 1,75 physische breit und würde als weiches Band über zwei
/// Pixelreihen gezeichnet. Die Linie rundet ihre Stärke deshalb auf ganze
/// physische Pixel und zeichnet ohne Kantenglättung.
///
/// Das Verhältnis kommt aus der `MediaQuery`, die unter der Skalierung die
/// Pixeldichte mal Skalierung meldet. Ändert sich die Skalierung, baut die
/// Linie neu und malt neu, auch in einer zwischengespeicherten Schicht.
///
/// Bis HUM-356 zeichnete sie `Divider` aus `shadcn_flutter`; der kennt kein
/// Pixelraster und nimmt seine Stärke in logischen Pixeln.
class HHairline extends StatelessWidget {
  /// Creates a hairline.
  const HHairline({
    this.vertical = false,
    this.color,
    this.length,
    this.strong = false,
    super.key,
  });

  /// True for a vertical rule, false for a horizontal one.
  final bool vertical;

  /// Overrides the line colour of the theme.
  final Color? color;

  /// Length along the line. Null fills the parent, which therefore has to
  /// constrain that axis.
  final double? length;

  /// Uses the emphasised line colour.
  final bool strong;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color resolved =
        color ?? (strong ? tokens.colors.lineStrong : tokens.colors.line);
    return SizedBox(
      width: vertical ? HSize.hairline : (length ?? double.infinity),
      height: vertical ? (length ?? double.infinity) : HSize.hairline,
      child: _SnappedLine(
        color: resolved,
        vertical: vertical,
        ratio:
            MediaQuery.maybeDevicePixelRatioOf(context) ??
            View.maybeOf(context)?.devicePixelRatio ??
            1,
      ),
    );
  }
}

class _SnappedLine extends LeafRenderObjectWidget {
  const _SnappedLine({
    required this.color,
    required this.vertical,
    required this.ratio,
  });

  final Color color;
  final bool vertical;
  final double ratio;

  @override
  RenderHairline createRenderObject(BuildContext context) =>
      RenderHairline(color: color, vertical: vertical, ratio: ratio);

  @override
  void updateRenderObject(BuildContext context, RenderHairline renderObject) {
    renderObject
      ..color = color
      ..vertical = vertical
      ..ratio = ratio;
  }
}

/// Zeichnet eine Haarlinie in ganzen physischen Pixeln.
///
/// Füllt die Fläche, die ihm sein Elternteil gibt. Bei einem ganzzahligen
/// [ratio] zeichnet es wie bisher eine Linie durch die Mitte; bei einem
/// gebrochenen rundet es die Stärke quer zur Linie auf ganze physische
/// Pixel, mindestens einen, und zeichnet ohne Kantenglättung am Anfang der
/// Fläche.
///
/// **Bewusst ohne Lage.** Die Linie fragt nicht, wo sie auf dem Schirm
/// steht: Eine Schicht, die eine Liste beim Rollen verschiebt, ohne neu zu
/// malen, ließe eine daraus gerechnete Lage veralten. Ohne Kantenglättung
/// deckt ein Rechteck von genau einem physischen Pixel Stärke genau eine
/// Pixelreihe, gleich wo es beginnt. Das gilt für die Rasterung ohne
/// Kantenglättung von Skia; ein Renderer, der Kanten immer glättet
/// (Impeller mit MSAA), zeichnet an einer gebrochenen Lage weiter ein
/// schmales Band (`docs/DESIGN.md`, HHairline).
class RenderHairline extends RenderBox {
  /// Creates the line.
  RenderHairline({
    required this._color,
    required this._vertical,
    required this._ratio,
  });

  /// The colour of the line.
  Color get color => _color;
  Color _color;
  set color(Color value) {
    if (value != _color) {
      _color = value;
      markNeedsPaint();
    }
  }

  /// True for a vertical line.
  bool get vertical => _vertical;
  bool _vertical;
  set vertical(bool value) {
    if (value != _vertical) {
      _vertical = value;
      markNeedsPaint();
    }
  }

  /// Physical pixels per logical pixel where the line stands, the scaling of
  /// the application included.
  double get ratio => _ratio;
  double _ratio;
  set ratio(double value) {
    if (value != _ratio) {
      _ratio = value;
      markNeedsPaint();
    }
  }

  @override
  bool get sizedByParent => true;

  @override
  Size computeDryLayout(BoxConstraints constraints) =>
      constraints.hasBoundedWidth && constraints.hasBoundedHeight
      ? constraints.biggest
      : constraints.smallest;

  /// Whether a logical pixel is a whole number of physical ones here.
  bool get integral =>
      _ratio <= 0 || (_ratio - _ratio.roundToDouble()).abs() < 1e-9;

  /// The thickness across the line in logical pixels for an [extent] of
  /// logical pixels: whole physical pixels, at least one.
  double thicknessFor(double extent) {
    final double pixels = (extent * _ratio).roundToDouble();
    return (pixels < 1 ? 1 : pixels) / _ratio;
  }

  @override
  void paint(PaintingContext context, Offset offset) {
    if (integral) {
      // Wie vor HUM-356 der `Divider`: eine Linie durch die Mitte mit
      // eckigem Ende, Pixel für Pixel dasselbe Bild.
      final Paint stroke = Paint()
        ..color = _color
        ..strokeWidth = _vertical ? size.width : size.height
        ..strokeCap = StrokeCap.square;
      final Offset start = _vertical
          ? offset + Offset(size.width / 2, 0)
          : offset + Offset(0, size.height / 2);
      final Offset end = _vertical
          ? offset + Offset(size.width / 2, size.height)
          : offset + Offset(size.width, size.height / 2);
      context.canvas.drawLine(start, end, stroke);
      return;
    }
    final Paint paint = Paint()
      ..color = _color
      ..isAntiAlias = false;
    final Rect line = _vertical
        ? Rect.fromLTWH(
            offset.dx,
            offset.dy,
            thicknessFor(size.width),
            size.height,
          )
        : Rect.fromLTWH(
            offset.dx,
            offset.dy,
            size.width,
            thicknessFor(size.height),
          );
    context.canvas.drawRect(line, paint);
  }
}
