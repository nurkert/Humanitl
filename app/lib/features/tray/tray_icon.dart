/// The tray icon, drawn rather than loaded (HUM-034).
///
/// HUM-034 proposes a set of PNG files plus a script that renders them from
/// SVG. Drawing them here instead removes the generator, the assets and the
/// risk that icon and design token drift apart: the icon takes its colours
/// from `HColors`, so a change to the held amber reaches the tray with
/// everything else.
///
/// The colours are the dark ones. The tray is not part of the themed widget
/// tree, and nothing tells an application whether the panel it sits in is
/// dark or light; the state colours were chosen to carry over both ladders
/// (BACKLOG.md 5), and the digit is drawn on the state colour, never on the
/// panel.
library;

import 'dart:async';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/painting.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'desktop_ports.dart';

/// The two edge lengths the tray gets, in pixels.
///
/// 22 is the usual panel height on X11 panels, 44 is the same at a scale of
/// two. Hosts pick the one they want; a host that scales picks the larger.
const List<int> trayIconSizes = <int>[22, 44];

/// Above this many held requests the icon shows `9+` instead of a number.
///
/// Four glyphs in twenty-two pixels are not a number any more, they are a
/// smudge, and the exact figure is one hover away in the tooltip.
const int trayCountCap = 9;

/// One rendered icon: ARGB32 bytes, as StatusNotifierItem wants them.
class TrayPixmap {
  /// Creates a pixmap.
  const TrayPixmap({
    required this.width,
    required this.height,
    required this.argb,
  });

  /// Width in pixels.
  final int width;

  /// Height in pixels.
  final int height;

  /// The pixels, four bytes each, alpha first, straight (not premultiplied).
  final Uint8List argb;
}

/// Draws the icon for [state] and [count] in every size of [trayIconSizes].
Future<List<TrayPixmap>> renderTrayIcon({
  required TrayIconState state,
  required int count,
  List<int> sizes = trayIconSizes,
}) async {
  final List<TrayPixmap> pixmaps = <TrayPixmap>[];
  for (final int size in sizes) {
    pixmaps.add(await _render(state: state, count: count, size: size));
  }
  return pixmaps;
}

/// The label the icon carries, or empty when it carries none.
String trayIconLabel(TrayIconState state, int count) => switch (state) {
  TrayIconState.idle => '',
  TrayIconState.held || TrayIconState.alert =>
    count <= 0 ? '' : (count > trayCountCap ? '$trayCountCap+' : '$count'),
  TrayIconState.offline => '?',
};

/// The colour the icon is drawn in.
Color trayIconColor(TrayIconState state) => switch (state) {
  TrayIconState.idle => HColors.fg2,
  TrayIconState.held => HColors.held,
  TrayIconState.alert => HColors.blocked,
  TrayIconState.offline => HColors.timedOut,
};

/// True while the icon is a filled chip rather than an outlined one.
///
/// Filled means "something is waiting for you". Idle and offline are outlines:
/// one because nothing waits, the other because nothing is known, and neither
/// deserves a saturated area at the edge of the eye (`docs/UX.md` 3.3).
bool trayIconIsFilled(TrayIconState state) =>
    state == TrayIconState.held || state == TrayIconState.alert;

Future<TrayPixmap> _render({
  required TrayIconState state,
  required int count,
  required int size,
}) async {
  final ui.PictureRecorder recorder = ui.PictureRecorder();
  final Canvas canvas = Canvas(recorder);
  final double edge = size.toDouble();
  final Color color = trayIconColor(state);
  final bool filled = trayIconIsFilled(state);

  // One pixel of air on every side at 22 px, two at 44 px: an icon that
  // touches the edge of its cell looks bigger than its neighbours.
  final double inset = edge / 22;
  final double stroke = edge / 11;
  final Rect chip = Rect.fromLTWH(
    inset,
    inset,
    edge - 2 * inset,
    edge - 2 * inset,
  );
  final RRect rounded = RRect.fromRectAndRadius(
    chip,
    Radius.circular(edge * HRadius.control / 22),
  );

  if (filled) {
    canvas.drawRRect(rounded, Paint()..color = color);
  } else {
    canvas.drawRRect(
      rounded.deflate(stroke / 2),
      Paint()
        ..color = color
        ..style = PaintingStyle.stroke
        ..strokeWidth = stroke,
    );
  }

  final String label = trayIconLabel(state, count);
  if (label.isNotEmpty) {
    _drawLabel(
      canvas: canvas,
      label: label,
      box: chip,
      color: filled ? HColors.bg0 : color,
    );
  } else if (state == TrayIconState.alert) {
    _drawCross(canvas: canvas, box: chip, color: HColors.bg0, width: stroke);
  }

  final ui.Picture picture = recorder.endRecording();
  final ui.Image image = await picture.toImage(size, size);
  picture.dispose();
  final ByteData? raw = await image.toByteData(
    // Straight, not premultiplied: ARGB32 on the StatusNotifierItem wire is
    // straight alpha, and a premultiplied buffer read as a straight one draws
    // every antialiased edge too dark.
    format: ui.ImageByteFormat.rawStraightRgba,
  );
  image.dispose();
  return TrayPixmap(
    width: size,
    height: size,
    argb: _toArgb(raw?.buffer.asUint8List() ?? Uint8List(size * size * 4)),
  );
}

void _drawLabel({
  required Canvas canvas,
  required String label,
  required Rect box,
  required Color color,
}) {
  // Two characters have to fit into the same box as one, so the size falls
  // with the length instead of the text being clipped.
  final double fontSize = box.height * (label.length > 1 ? 0.62 : 0.78);
  final ui.ParagraphBuilder builder =
      ui.ParagraphBuilder(
          ui.ParagraphStyle(
            textAlign: TextAlign.center,
            fontSize: fontSize,
            fontWeight: FontWeight.w600,
            height: 1,
          ),
        )
        ..pushStyle(ui.TextStyle(color: color, fontSize: fontSize, height: 1))
        ..addText(label);
  final ui.Paragraph paragraph = builder.build()
    ..layout(ui.ParagraphConstraints(width: box.width));
  canvas.drawParagraph(
    paragraph,
    Offset(box.left, box.top + (box.height - paragraph.height) / 2),
  );
  paragraph.dispose();
}

void _drawCross({
  required Canvas canvas,
  required Rect box,
  required Color color,
  required double width,
}) {
  final Rect arms = box.deflate(box.width / 4);
  final Paint paint = Paint()
    ..color = color
    ..strokeWidth = width
    ..strokeCap = StrokeCap.round;
  canvas
    ..drawLine(arms.topLeft, arms.bottomRight, paint)
    ..drawLine(arms.topRight, arms.bottomLeft, paint);
}

/// Reorders RGBA bytes into ARGB, which is what the tray protocol carries.
Uint8List _toArgb(Uint8List rgba) {
  final Uint8List argb = Uint8List(rgba.length);
  for (int i = 0; i + 3 < rgba.length; i += 4) {
    argb[i] = rgba[i + 3];
    argb[i + 1] = rgba[i];
    argb[i + 2] = rgba[i + 1];
    argb[i + 3] = rgba[i + 2];
  }
  return argb;
}
