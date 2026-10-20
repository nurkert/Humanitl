/// The application icon as a widget: the fist on the yellow tile (HUM-259).
///
/// Drawn as a path rather than loaded, because the app has no SVG package and
/// takes on no dependency for one picture. The geometry is the one of
/// `packaging/deb/humanitl.svg`, coordinate for coordinate, so the mark in the
/// header and the icon in the menu are the same drawing. Whoever changes one
/// changes the other in the same commit.
///
/// The fist follows "hand-fist" from Font Awesome Free 6.6.0 by Fonticons,
/// Inc. (CC BY 4.0, https://fontawesome.com/license/free), index and middle
/// finger reshaped; credits in `THIRD_PARTY.md`.
///
/// The colours are the icon's own and not theme tokens: an application icon
/// looks the same on a dark and on a light surface, like it does in the menu
/// of the desktop.
library;

import 'package:flutter/widgets.dart';

/// Top of the tile's vertical gradient.
const Color appMarkTileTop = Color(0xFFEBC05A);

/// Bottom of the tile's vertical gradient.
const Color appMarkTileBottom = Color(0xFFD49E34);

/// The fist.
const Color appMarkFist = Color(0xFF14161C);

/// The application icon at [size] logical pixels square.
///
/// Decorative: it carries no semantics, the product name stands next to it
/// wherever it is used.
class AppMark extends StatelessWidget {
  /// Creates the mark.
  const AppMark({this.size = 16, super.key});

  /// Edge length in logical pixels.
  final double size;

  @override
  Widget build(BuildContext context) => SizedBox.square(
    dimension: size,
    child: const CustomPaint(painter: AppMarkPainter()),
  );
}

/// Paints the icon into whatever square it is given.
class AppMarkPainter extends CustomPainter {
  /// Creates the painter.
  const AppMarkPainter();

  /// Edge length of the SVG's view box, the unit of every number below.
  static const double viewBox = 256;

  @override
  void paint(Canvas canvas, Size size) {
    final double edge = size.shortestSide;
    canvas
      ..save()
      ..scale(edge / viewBox);

    // <rect x="8" y="8" width="240" height="240" rx="56" fill="url(#tile)"/>
    const Rect tile = Rect.fromLTWH(8, 8, 240, 240);
    canvas.drawRRect(
      RRect.fromRectAndRadius(tile, const Radius.circular(56)),
      Paint()
        ..shader = const LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: <Color>[appMarkTileTop, appMarkTileBottom],
        ).createShader(tile),
    );

    // transform="translate(128 130) scale(0.34) translate(-224 -256)"
    canvas
      ..translate(128, 130)
      ..scale(0.34)
      ..translate(-224, -256);
    final Paint fist = Paint()..color = appMarkFist;
    // The four fingers, each a rect with rx of half its width.
    for (final Rect finger in const <Rect>[
      Rect.fromLTWH(64, 32, 64, 124),
      Rect.fromLTWH(160, 0, 64, 156),
      Rect.fromLTWH(256, 32, 64, 160),
      Rect.fromLTWH(352, 96, 64, 128),
    ]) {
      canvas.drawRRect(
        RRect.fromRectAndRadius(finger, const Radius.circular(32)),
        fist,
      );
    }
    canvas
      ..drawPath(_palm, fist)
      ..restore();
  }

  @override
  bool shouldRepaint(AppMarkPainter oldDelegate) => false;
}

/// Thumb and palm: the `<path>` of the SVG, its relative commands resolved
/// to absolute coordinates and `s` to its reflected control point.
final Path _palm = Path()
  ..moveTo(256, 216)
  ..lineTo(256, 215.4)
  ..cubicTo(265.4, 220.8, 276.3, 224, 288, 224)
  ..cubicTo(301.2, 224, 313.4, 220, 323.6, 213.2)
  ..cubicTo(332.3, 238.1, 356.1, 256, 384, 256)
  ..cubicTo(395.7, 256, 406.6, 252.9, 416, 247.4)
  ..lineTo(416, 256)
  ..cubicTo(416, 308.3, 390.9, 354.8, 352, 384)
  ..lineTo(352, 480)
  ..cubicTo(352, 497.7, 337.7, 512, 320, 512)
  ..lineTo(160, 512)
  ..cubicTo(142.3, 512, 128, 497.7, 128, 480)
  ..lineTo(128, 401.6)
  ..cubicTo(110.7, 393.7, 94.8, 382.8, 81.1, 369.1)
  ..lineTo(69.5, 357.5)
  ..cubicTo(45.5, 333.5, 32, 300.9, 32, 267)
  ..lineTo(32, 240)
  ..cubicTo(32, 204.7, 60.7, 176, 96, 176)
  ..lineTo(184, 176)
  ..cubicTo(206.1, 176, 224, 193.9, 224, 216)
  ..cubicTo(224, 238.1, 206.1, 256, 184, 256)
  ..lineTo(128, 256)
  ..cubicTo(119.2, 256, 112, 263.2, 112, 272)
  ..cubicTo(112, 280.8, 119.2, 288, 128, 288)
  ..lineTo(184, 288)
  ..cubicTo(223.8, 288, 256, 255.8, 256, 216)
  ..close();
