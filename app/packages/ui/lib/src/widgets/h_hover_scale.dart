import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

import '../tokens/motion.dart';
import 'h_animated_fill.dart';

/// Die Bewegung einer überfahrenen Zeile: eine leichte Skalierung um ihre
/// Mitte, nie eine Verschiebung (HUM-406).
///
/// Vorher kam der Hover einer Liste schlagartig, und an der Regelliste
/// sprang die Schrift der Zeile zwei Pixel nach unten, sobald der
/// Aktionsslot aufging: Wer mit der Maus durch die Liste fuhr, sah jede
/// überfahrene Zeile zucken. Dieses Widget ist die eine Stelle, an der die
/// Antwort steht:
///
/// * **Animiert.** Der Übergang läuft über [HMotion.press] auf
///   [HMotion.enter], hin wie zurück.
/// * **Nur Skalierung, nur gemalt.** Die Zeile wächst gleichmäßig um ihre
///   Mitte, bis ihre Seitenkanten [HMotion.hoverGrow] weiter außen stehen.
///   Layoutgröße, Lage der Nachbarn und Trefferfläche ändern sich nicht: das
///   Widget transformiert nur beim Malen. Eine Trefferfläche, die mitwüchse,
///   ließe den Zeiger an der Kante zwischen zwei Zeilen hin- und herspringen.
///   Der gemalte Rand von bis zu [HMotion.hoverGrow] je Seite trifft deshalb
///   mit Absicht nichts, und wer während des Hovers die Lage eines Nachfahren
///   misst (`localToGlobal`, `getRect`), bekommt sie mit der Skalierung.
/// * **Reduzierte Bewegung.** Mit `MediaQuery.disableAnimations` bleibt die
///   Zeile stehen; übrig bleibt der Farbwechsel, den der Aufrufer ohnehin
///   über [HAnimatedFill] führt (`docs/UX.md` 2.10: weniger Weg, nicht
///   weniger Rückmeldung).
///
/// Die Füllung gehört nicht hierher: Hover, Auswahl und Fokus haben je eine
/// eigene Farbe, und die kennt nur die Zeile.
class HHoverScale extends StatefulWidget {
  /// Scales [child] slightly while [hovered] holds.
  const HHoverScale({required this.hovered, required this.child, super.key});

  /// Ob der Zeiger gerade auf der Zeile steht.
  final bool hovered;

  /// Die Zeile.
  final Widget child;

  @override
  State<HHoverScale> createState() => _HHoverScaleState();
}

class _HHoverScaleState extends State<HHoverScale>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: HMotion.press,
    value: widget.hovered ? 1 : 0,
  );

  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _controller,
    curve: HMotion.enter,
  );

  @override
  void didUpdateWidget(HHoverScale oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.hovered != oldWidget.hovered) {
      widget.hovered ? _controller.forward() : _controller.reverse();
    }
  }

  @override
  void dispose() {
    _curve.dispose();
    _controller.dispose();
    super.dispose();
  }

  /// Ruhe unter reduzierter Bewegung: kein Weg, also kein Übergang.
  static const Animation<double> _still = AlwaysStoppedAnimation<double>(0);

  @override
  Widget build(BuildContext context) =>
      // Immer derselbe Baum, nur die Animation wechselt: Schaltet jemand
      // „Bewegung reduzieren" um, behalten die Zeilen ihren Zustand, statt
      // unter einem anderen Elternteil neu gebaut zu werden.
      _HoverScale(
        progress: HReducedMotion.of(context) ? _still : _curve,
        child: widget.child,
      );
}

class _HoverScale extends SingleChildRenderObjectWidget {
  const _HoverScale({required this.progress, required super.child});

  final Animation<double> progress;

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _RenderHoverScale(progress);

  @override
  void updateRenderObject(BuildContext context, _RenderHoverScale render) {
    render.progress = progress;
  }
}

/// Malt das Kind skaliert um seine Mitte und lässt Layout und Treffertest in
/// Ruhe.
class _RenderHoverScale extends RenderProxyBox {
  _RenderHoverScale(this._progress);

  Animation<double> _progress;

  set progress(Animation<double> value) {
    if (identical(value, _progress)) {
      return;
    }
    if (attached) {
      _progress.removeListener(markNeedsPaint);
      value.addListener(markNeedsPaint);
    }
    _progress = value;
    markNeedsPaint();
  }

  /// Der Faktor dieses Frames: die Seitenkanten gehen um höchstens
  /// [HMotion.hoverGrow] nach außen.
  double get _scale {
    final double width = size.width;
    if (width <= 0) {
      return 1;
    }
    return 1 + 2 * HMotion.hoverGrow * _progress.value / width;
  }

  @override
  void attach(PipelineOwner owner) {
    super.attach(owner);
    _progress.addListener(markNeedsPaint);
  }

  @override
  void detach() {
    _progress.removeListener(markNeedsPaint);
    super.detach();
  }

  /// Die Transformation dieses Frames, oder null in Ruhe: eine Skalierung um
  /// die Mitte. Malen und [applyPaintTransform] lesen beide nur sie, damit
  /// gemessene und gemalte Lage nie auseinanderlaufen.
  Matrix4? get _transform {
    final double scale = _scale;
    if (scale == 1) {
      return null;
    }
    final Offset centre = size.center(Offset.zero);
    return Matrix4.identity()
      ..translateByDouble(centre.dx, centre.dy, 0, 1)
      ..scaleByDouble(scale, scale, 1, 1)
      ..translateByDouble(-centre.dx, -centre.dy, 0, 1);
  }

  @override
  void paint(PaintingContext context, Offset offset) {
    final RenderBox? child = this.child;
    if (child == null) {
      return;
    }
    final Matrix4? transform = _transform;
    if (transform == null) {
      layer = null;
      context.paintChild(child, offset);
      return;
    }
    layer = context.pushTransform(
      needsCompositing,
      offset,
      transform,
      (PaintingContext context, Offset offset) =>
          context.paintChild(child, offset),
      oldLayer: layer is TransformLayer ? layer! as TransformLayer : null,
    );
  }

  @override
  void applyPaintTransform(RenderBox child, Matrix4 transform) {
    final Matrix4? own = _transform;
    if (own != null) {
      transform.multiply(own);
    }
  }
}
