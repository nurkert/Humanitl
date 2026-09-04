import 'package:flutter/widgets.dart';

import '../tokens/motion.dart';

/// Eine Füllung, die ihre Dauer behält.
///
/// `AnimatedContainer` und jedes andere implizit animierte Widget bauen ihren
/// Controller ohne `animationBehavior`. Sobald die Plattform
/// `disableAnimations` meldet — und der Linux-Embedder meldet es —, skaliert
/// Flutter jede Dauer auf fünf Prozent: aus den 120 ms der Tastenfüllung
/// werden 6 ms, und das Control steht voll gefüllt da, bevor jemand den
/// Druck als Rückmeldung lesen kann. `docs/UX.md` 2.10 nennt die
/// Tastenfüllung namentlich unter dem, was seine volle Dauer behält:
/// reduzierte Bewegung heißt weniger Weg, nicht weniger Rückmeldung.
///
/// Deshalb dieses Widget. Es animiert genau eine Farbe, mit
/// [AnimationBehavior.preserve], und reicht den Zwischenwert an [builder].
/// Wer Größe oder Lage animieren will, ist hier falsch: eine Strecke gehört
/// unter [HReducedMotion], eine Rückmeldung nicht.
class HAnimatedFill extends StatefulWidget {
  /// Creates a fill that animates to [color].
  const HAnimatedFill({
    required this.color,
    required this.builder,
    this.duration = HMotion.press,
    this.curve = HMotion.enter,
    super.key,
  });

  /// Die Zielfarbe. Ein neuer Wert startet den Übergang.
  final Color color;

  /// Wie lange der Übergang dauert.
  final Duration duration;

  /// Die Kurve des Übergangs.
  final Curve curve;

  /// Zeichnet das Control mit der Farbe dieses Frames.
  final Widget Function(BuildContext context, Color color) builder;

  @override
  State<HAnimatedFill> createState() => _HAnimatedFillState();
}

class _HAnimatedFillState extends State<HAnimatedFill>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: widget.duration,
    value: 1,
    animationBehavior: AnimationBehavior.preserve,
  );

  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _controller,
    curve: widget.curve,
  );

  /// Die Farbe, von der der laufende Übergang ausgeht.
  late Color _from = widget.color;

  Color get _value => Color.lerp(_from, widget.color, _curve.value)!;

  @override
  void didUpdateWidget(HAnimatedFill oldWidget) {
    super.didUpdateWidget(oldWidget);
    _controller.duration = widget.duration;
    if (widget.color != oldWidget.color) {
      // Von dort weiter, wo der letzte Übergang steht: ein Zeiger, der über
      // ein Control fährt und sofort wieder herunter, springt sonst. Der
      // Ausgangspunkt rechnet mit der **alten** Zielfarbe: in
      // `didUpdateWidget` ist `widget` schon die neue, und wer die nimmt,
      // startet den Übergang dort, wo er enden soll.
      _from = Color.lerp(_from, oldWidget.color, _curve.value)!;
      _controller.forward(from: 0);
    }
  }

  @override
  void dispose() {
    _curve.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: _curve,
    builder: (BuildContext context, Widget? child) =>
        widget.builder(context, _value),
  );
}
