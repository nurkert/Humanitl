/// The small picture at the head of each step of the guided agent setup
/// (GitHub #111).
///
/// Drawn from the tokens of the design system and nothing else: hairlines
/// in the line colours, the structure in `fg2`, and one part in the colour
/// of the step's state, the part the step is about. No image file, no icon
/// font, no brand art beyond the app's own mark.
///
/// - Source: the machine on the left, the depot box on the right, the
///   sandbox below; the line runs from the source in use.
/// - Install: the depot box with the arrow into it and a lock for the
///   sign-in.
/// - Inference: the agent and its model server inside the dashed edge of
///   the own network; with the agent's own providers the line leaves it.
/// - Update: three versions stacked, the newest on top.
/// - Ready: the app's mark on its tile.
library;

import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';

import '../../../../core/ui/app_mark.dart';
import '../../../../core/ui/ui.dart';
import '../../setup/setup_steps.dart';

/// Width of the tile.
const double setupEmblemWidth = 88;

/// Height of the tile.
const double setupEmblemHeight = 64;

/// The tile with the picture of [step].
class SetupEmblem extends StatelessWidget {
  /// Creates the picture of [step] in [state]; [variant] chooses the form
  /// within the step: the depot as source, the agent's own providers.
  const SetupEmblem({
    required this.step,
    required this.state,
    this.variant = false,
    super.key,
  });

  /// The step.
  final SetupStepId step;

  /// Its state, which colours the one part the step is about.
  final SetupStepState state;

  /// The depot as source, or the agent's own providers as inference.
  final bool variant;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color focus = switch (state) {
      SetupStepState.done => tokens.state.allowed,
      SetupStepState.action => tokens.colors.warning,
      SetupStepState.running || SetupStepState.blocked => tokens.colors.fg1,
    };
    return ExcludeSemantics(
      child: Container(
        key: Key('agent-setup-emblem-${step.name}'),
        width: setupEmblemWidth,
        height: setupEmblemHeight,
        decoration: BoxDecoration(
          color: tokens.colors.bg2,
          borderRadius: BorderRadius.circular(tokens.radii.card),
          border: Border.all(color: tokens.colors.line),
        ),
        child: step == SetupStepId.ready
            ? const Center(child: AppMark(size: 32))
            : CustomPaint(
                painter: _EmblemPainter(
                  step: step,
                  variant: variant,
                  structure: tokens.colors.fg2,
                  quiet: tokens.colors.lineStrong,
                  surface: tokens.colors.bg1,
                  focus: focus,
                  stroke: HSize.ringStroke,
                ),
              ),
      ),
    );
  }
}

class _EmblemPainter extends CustomPainter {
  const _EmblemPainter({
    required this.step,
    required this.variant,
    required this.structure,
    required this.quiet,
    required this.surface,
    required this.focus,
    required this.stroke,
  });

  final SetupStepId step;
  final bool variant;
  final Color structure;
  final Color quiet;
  final Color surface;
  final Color focus;
  final double stroke;

  Paint _line(Color color) => Paint()
    ..color = color
    ..style = PaintingStyle.stroke
    ..strokeWidth = stroke
    ..strokeCap = StrokeCap.round
    ..strokeJoin = StrokeJoin.round;

  Paint get _fill => Paint()..color = surface;

  @override
  void paint(Canvas canvas, Size size) {
    switch (step) {
      case SetupStepId.source:
        _source(canvas);
      case SetupStepId.install:
        _install(canvas);
      case SetupStepId.inference:
        _inference(canvas);
      case SetupStepId.update:
        _update(canvas);
      case SetupStepId.ready:
        break;
    }
  }

  void _box(Canvas canvas, Rect rect, Color color, {bool lid = false}) {
    final RRect r = RRect.fromRectAndRadius(rect, const Radius.circular(3));
    canvas
      ..drawRRect(r, _fill)
      ..drawRRect(r, _line(color));
    if (lid) {
      canvas.drawLine(
        Offset(rect.left, rect.top + 5),
        Offset(rect.right, rect.top + 5),
        _line(color),
      );
    }
  }

  void _source(Canvas canvas) {
    final Color host = variant ? structure : focus;
    final Color depot = variant ? focus : structure;
    // Der Rechner: Bildschirm mit Fuß.
    _box(canvas, const Rect.fromLTWH(14, 12, 22, 15), host);
    canvas.drawLine(const Offset(20, 31), const Offset(30, 31), _line(host));
    // Das Depot: eine Kiste mit Deckel.
    _box(canvas, const Rect.fromLTWH(52, 12, 22, 19), depot, lid: true);
    // Die Sandbox unten, gestrichelt umrandet.
    _dashed(canvas, const Rect.fromLTWH(34, 42, 20, 12), quiet);
    final Offset from = variant ? const Offset(63, 31) : const Offset(25, 33);
    canvas.drawLine(from, const Offset(44, 42), _line(focus));
  }

  void _install(Canvas canvas) {
    _box(canvas, const Rect.fromLTWH(30, 26, 28, 26), focus, lid: true);
    final Paint arrow = _line(structure);
    canvas
      ..drawLine(const Offset(44, 8), const Offset(44, 22), arrow)
      ..drawLine(const Offset(39, 17), const Offset(44, 22), arrow)
      ..drawLine(const Offset(49, 17), const Offset(44, 22), arrow);
    // Das Schloss der Anmeldung rechts unten an der Kiste.
    _box(canvas, const Rect.fromLTWH(60, 40, 12, 10), structure);
    canvas.drawArc(
      const Rect.fromLTWH(62, 34, 8, 10),
      math.pi,
      math.pi,
      false,
      _line(structure),
    );
  }

  void _inference(Canvas canvas) {
    // Der Rand des eigenen Netzes.
    _dashed(canvas, const Rect.fromLTWH(8, 10, 52, 44), quiet);
    // Der Agent.
    _box(canvas, const Rect.fromLTWH(14, 24, 14, 14), structure);
    if (!variant) {
      // Der Modellserver: zwei Einschübe.
      _box(canvas, const Rect.fromLTWH(38, 20, 16, 22), focus);
      canvas
        ..drawLine(const Offset(42, 27), const Offset(50, 27), _line(focus))
        ..drawLine(const Offset(42, 34), const Offset(50, 34), _line(focus))
        ..drawLine(const Offset(28, 31), const Offset(38, 31), _line(focus));
      return;
    }
    // Die eigenen Provider: die Linie verlässt das Netz.
    canvas
      ..drawLine(const Offset(28, 31), const Offset(74, 31), _line(focus))
      ..drawLine(const Offset(69, 26), const Offset(74, 31), _line(focus))
      ..drawLine(const Offset(69, 36), const Offset(74, 31), _line(focus));
  }

  void _update(Canvas canvas) {
    _box(canvas, const Rect.fromLTWH(22, 40, 36, 10), quiet);
    _box(canvas, const Rect.fromLTWH(18, 27, 44, 10), structure);
    _box(canvas, const Rect.fromLTWH(14, 14, 52, 10), focus);
    final Paint arrow = _line(focus);
    canvas
      ..drawLine(const Offset(74, 30), const Offset(74, 14), arrow)
      ..drawLine(const Offset(70, 18), const Offset(74, 14), arrow)
      ..drawLine(const Offset(78, 18), const Offset(74, 14), arrow);
  }

  void _dashed(Canvas canvas, Rect rect, Color color) {
    final ui.Path outline = ui.Path()
      ..addRRect(RRect.fromRectAndRadius(rect, const Radius.circular(4)));
    final Paint paint = _line(color);
    for (final ui.PathMetric metric in outline.computeMetrics()) {
      for (double at = 0; at < metric.length; at += 6) {
        canvas.drawPath(metric.extractPath(at, at + 3), paint);
      }
    }
  }

  @override
  bool shouldRepaint(_EmblemPainter old) =>
      old.step != step ||
      old.variant != variant ||
      old.structure != structure ||
      old.quiet != quiet ||
      old.surface != surface ||
      old.focus != focus ||
      old.stroke != stroke;
}
