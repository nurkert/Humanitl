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
/// the request arrived and 0.0 at the deadline. It never turns red, because
/// red means blocked.
///
/// Die verbrauchte Zeit ist eine Lücke, keine Spur. Eine Spur bräuchte eine
/// eigene Farbe, die auf jeder Panelfläche zu sehen ist und gegen die der
/// Bogen trotzdem 3:1 erreicht; die Haarlinie war beides nicht — hell misst
/// sie gegen die Flächen 1,01:1 bis 1,19:1 und war damit auf keinem Panel
/// sichtbar. Der Bogen allein erreicht als Zustandsfarbe auf jeder der vier
/// Flächen beider Leitern seine 3:1, und die Lücke braucht kein Token
/// (`docs/UX.md` 9, Punkt 6).
///
/// Der Atem ist eine Flagge, keine Skala (`docs/UX.md` 2.7). Er sagt „jetzt
/// hinsehen", und zwar begrenzt: [HMotion.breatheCycles] Züge, wenn die
/// Restfrist [HMotion.breatheBelow] unterschreitet, dieselbe Zahl noch einmal
/// bei [HMotion.breatheBelowUrgent], danach Ruhe. Er läuft über
/// [Curves.easeInOut] — eine Dreieckwelle hat an beiden Enden eine Ecke, und
/// die liest das Auge als Blinken — und nimmt das Glyph nie unter
/// [HMotion.breatheMinOpacity]; sonst wäre die dringendste Anfrage die am
/// schlechtesten sichtbare. Jeder Zug beginnt bei voller Deckkraft.
///
/// Unter reduzierter Bewegung entfällt die Schleife nicht ersatzlos: an ihre
/// Stelle tritt ein zweiter, ruhender Ring bei [HMotion.reducedRingAlpha]. Die
/// Schwelle ist Information, und wer Animationen abgeschaltet hat, darf sie
/// nicht verlieren (2.10).
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
  // Ohne `AnimationBehavior.preserve`, und das ist hier richtig: der Atem ist
  // eine Flagge, keine Frist. Wer Animationen abgeschaltet hat, bekommt statt
  // der Schleife den ruhenden zweiten Ring, also kürzt die Plattform nichts
  // ab, was etwas absichert (`docs/UX.md` 2.10).
  late final AnimationController _breath = AnimationController(
    vsync: this,
    duration: HMotion.breathe,
  );

  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _breath,
    curve: Curves.easeInOut,
  );

  /// Wie viele der beiden Schwellen schon ausgelöst haben. Zwei Ereignisse,
  /// dann Ruhe: ein endloser Puls im Augenwinkel nörgelt (`docs/UX.md` 2.7).
  int _thresholdsFired = 0;

  /// Wie viele Schwellen [progress] jetzt unterschreitet.
  int get _thresholdsReached {
    final double? progress = widget.progress;
    if (progress == null) {
      return 0;
    }
    if (progress <= HMotion.breatheBelowUrgent) {
      return 2;
    }
    if (progress <= HMotion.breatheBelow) {
      return 1;
    }
    return 0;
  }

  @override
  void initState() {
    super.initState();
    // Eine Zeile, die schon unter der Schwelle ankommt, hat den Übergang
    // nicht gezeigt; sie holt ihn nicht nach.
    _thresholdsFired = _thresholdsReached;
  }

  @override
  void didUpdateWidget(HStateGlyph oldWidget) {
    super.didUpdateWidget(oldWidget);
    _syncBreathing();
  }

  void _syncBreathing() {
    final int reached = _thresholdsReached;
    if (reached < _thresholdsFired) {
      // Die Frist ist neu gesetzt worden; die Schwellen gelten wieder.
      _thresholdsFired = reached;
      return;
    }
    if (reached <= _thresholdsFired) {
      return;
    }
    _thresholdsFired = reached;
    if (HReducedMotion.cycles(context, HMotion.breatheCycles) == 0) {
      return;
    }
    _breath
      ..stop()
      ..value = 0
      // `count` zählt Halbdurchläufe, nicht Atemzüge: `_RepeatingSimulation`
      // ist fertig, wenn `count` Läufe von 0 nach 1 vorbei sind, und bei einer
      // ungeraden Zahl endet der Controller auf 1,0 — also auf der geringsten
      // Deckkraft, und die dringendste Zeile bliebe dauerhaft die blasseste.
      // Drei Atemzüge sind sechs Halbdurchläufe und enden bei 0.
      ..repeat(reverse: true, count: HMotion.breatheCycles * 2);
  }

  @override
  void dispose() {
    _curve.dispose();
    _breath.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    // Bogen **und** Glyph nehmen die Flächenfarbe, nicht die Textvariante.
    // Ein Glyph ist eine Grafik und keine Schrift: seine Grenze ist die 3:1
    // aus `docs/UX.md` 6, und die hält die Flächenpalette auf jeder der vier
    // Flächen beider Leitern schon von sich aus. Die Textvariante wäre hier
    // sogar schädlich — `autoRule` trägt 60 % Deckkraft, und die einzige
    // Helligkeit, mit der es 4,5:1 erreicht, liegt so dicht an Weiß (dunkel)
    // beziehungsweise Schwarz (hell), dass der Ton verschwindet. Das Glyph
    // ist aber genau der Kanal, der die Farbe verdoppeln soll (3.3), und es
    // verlöre den Ton, den es verdoppelt.
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
              // Der ruhende Ersatz für den Atem: ein zweiter, stehender Ring,
              // sobald eine Schwelle gefallen ist und Bewegung aus ist.
              doubled: _thresholdsFired > 0 && HReducedMotion.of(context),
            ),
          ),
          glyph,
        ],
      );
    }
    glyph = FadeTransition(
      // Die Untergrenze ist ein Token: der Atem ist eine Flagge, keine Skala.
      opacity: _curve.drive(
        Tween<double>(begin: 1, end: HMotion.breatheMinOpacity),
      ),
      child: glyph,
    );
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
    this.doubled = false,
  });

  final double progress;
  final Color color;

  /// Ob der zweite, ruhende Ring gezeichnet wird, der unter reduzierter
  /// Bewegung an die Stelle des Atems tritt (`docs/UX.md` 2.10).
  final bool doubled;

  @override
  void paint(Canvas canvas, Size size) {
    final Rect rect = Offset.zero & size;
    final Rect circle = rect.deflate(HSize.ringStroke / 2);
    final Paint paint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = HSize.ringStroke
      ..strokeCap = StrokeCap.round
      ..color = color;
    // Nur der verbleibende Bogen. Die verbrauchte Strecke bleibt Lücke.
    canvas.drawArc(circle, -math.pi / 2, 2 * math.pi * progress, false, paint);
    if (doubled) {
      canvas.drawArc(
        circle.deflate(HSize.ringStroke + 1),
        0,
        2 * math.pi,
        false,
        paint..color = color.withValues(alpha: HMotion.reducedRingAlpha),
      );
    }
  }

  @override
  bool shouldRepaint(_CountdownRingPainter oldDelegate) =>
      oldDelegate.progress != progress ||
      oldDelegate.color != color ||
      oldDelegate.doubled != doubled;
}
