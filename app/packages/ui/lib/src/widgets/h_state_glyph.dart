import 'dart:math' as math;

import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/flow_state.dart';
import '../tokens/motion.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import 'h_glyph.dart';

/// The glyph of a flow state, optionally wrapped in a countdown ring.
///
/// [progress] is the *remaining* fraction of the hold budget, 1.0 right after
/// the request arrived and 0.0 at the deadline. Below
/// [HMotion.breatheBelow] the glyph breathes; it never turns red, because red
/// means blocked.
class HStateGlyph extends StatefulWidget {
  /// Creates a state glyph.
  const HStateGlyph({
    required this.state,
    this.size = HSize.glyph,
    this.progress,
    this.semanticsLabel,
    super.key,
  });

  /// Which state to draw.
  final HFlowState state;

  /// Edge length of the square the glyph occupies.
  final double size;

  /// Remaining fraction of the hold budget, or null for no ring.
  final double? progress;

  /// Screen-reader label. Callers pass the resolved translation of
  /// `state.l10nKey`.
  final String? semanticsLabel;

  @override
  State<HStateGlyph> createState() => _HStateGlyphState();
}

class _HStateGlyphState extends State<HStateGlyph>
    with SingleTickerProviderStateMixin {
  late final AnimationController _breath = AnimationController(
    vsync: this,
    duration: HMotion.breathe,
  );

  bool get _breathing {
    final double? progress = widget.progress;
    return progress != null && progress <= HMotion.breatheBelow;
  }

  @override
  void initState() {
    super.initState();
    _syncBreathing();
  }

  @override
  void didUpdateWidget(HStateGlyph oldWidget) {
    super.didUpdateWidget(oldWidget);
    _syncBreathing();
  }

  void _syncBreathing() {
    if (_breathing) {
      if (!_breath.isAnimating) {
        _breath.repeat(reverse: true);
      }
    } else if (_breath.isAnimating) {
      _breath
        ..stop()
        ..value = 0;
    }
  }

  @override
  void dispose() {
    _breath.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color color = tokens.stateColor(widget.state);
    final double? progress = widget.progress;
    final double inner = progress == null ? widget.size : widget.size * 0.62;
    Widget glyph = HGlyphIcon(
      widget.state.glyph,
      size: inner,
      color: color,
      accentColor: tokens.colors.accent,
    );
    if (progress != null) {
      glyph = Stack(
        alignment: Alignment.center,
        children: <Widget>[
          CustomPaint(
            size: Size.square(widget.size),
            painter: _CountdownRingPainter(
              progress: progress.clamp(0.0, 1.0),
              color: color,
              track: tokens.colors.line,
            ),
          ),
          glyph,
        ],
      );
    }
    if (_breathing) {
      glyph = AnimatedBuilder(
        animation: _breath,
        builder: (BuildContext context, Widget? child) =>
            Opacity(opacity: 1.0 - 0.55 * _breath.value, child: child),
        child: glyph,
      );
    }
    final Widget sized = SizedBox.square(dimension: widget.size, child: glyph);
    if (widget.semanticsLabel == null) {
      return ExcludeSemantics(child: sized);
    }
    return Semantics(label: widget.semanticsLabel, child: sized);
  }
}

class _CountdownRingPainter extends CustomPainter {
  _CountdownRingPainter({
    required this.progress,
    required this.color,
    required this.track,
  });

  final double progress;
  final Color color;
  final Color track;

  @override
  void paint(Canvas canvas, Size size) {
    final Rect rect = Offset.zero & size;
    final Rect circle = rect.deflate(HSize.ringStroke / 2);
    final Paint paint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = HSize.ringStroke
      ..strokeCap = StrokeCap.round
      ..color = track;
    canvas
      ..drawArc(circle, 0, 2 * math.pi, false, paint)
      ..drawArc(
        circle,
        -math.pi / 2,
        2 * math.pi * progress,
        false,
        paint..color = color,
      );
  }

  @override
  bool shouldRepaint(_CountdownRingPainter oldDelegate) =>
      oldDelegate.progress != progress ||
      oldDelegate.color != color ||
      oldDelegate.track != track;
}
