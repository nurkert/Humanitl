import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../theme/shadcn_theme.dart';
import '../tokens/colors.dart';
import '../tokens/tokens.dart';

/// Der Fokusring des Systems: zwei Pixel Akzent außerhalb des Controls, in
/// einem Frame.
///
/// `docs/UX.md` 6 legt drei Dinge fest, und dieses Widget ist die einzige
/// Stelle, an der sie stehen: der Ring liegt **außerhalb** des eigenen Rahmens,
/// er färbt nie einen vorhandenen Rahmen um, und er trägt nie die Farbe, mit
/// der das Control gefüllt ist. Dazu kommt aus 2.9: ein Fokusring animiert
/// nicht. Ein einblendender Ring liest sich als Eingabeverzögerung, und auf
/// dem Primärbutton maß der umgefärbte Rahmen gegen seine eigene Füllung
/// 1,00:1 — ein Tastaturnutzer sah dort keinen Unterschied.
///
/// Deshalb [gap]: der Ring ist der Akzent, und der Primärbutton ist mit dem
/// Akzent gefüllt. Läge der Ring auf der Kante der Füllung, stünde er wieder
/// bei rund 1:1 gegen sie, egal wie er gezeichnet ist. Die zwei Pixel Fläche
/// dazwischen sind der Kontrast: der Ring misst dort gegen die Fläche, auf
/// der das Control steht, nicht gegen das Control.
///
/// Der Abstand entsteht nur, wo er gebraucht wird. Wer [over] die deckende
/// Füllung seines Controls reicht, bekommt ihn, sobald der Ring gegen diese
/// Füllung unter [HColorDerivation.areaMinContrast] fällt — beim Primärbutton
/// also, sonst nirgends. Der Ring bliebe sonst überall zwei Pixel breiter und
/// verschöbe den Dichte-Rhythmus für ein Problem, das ein einziges Control
/// hat.
///
/// Zwei Formen, weil es zwei Geometrien gibt:
///
/// * [HFocusRing.new] reserviert den Platz für den Ring, ob er zu sehen ist
///   oder nicht. Nichts verschiebt sich, wenn der Fokus ankommt. Das ist die
///   Form für alles, was einen eigenen Rahmen hat: Button, Eingabefeld,
///   Segment, Kästchen.
/// * [HFocusRing.inline] zeichnet den Ring auf die eigene Kante des Kindes und
///   reserviert nichts. Das ist die Form für eine Zeile, die von Rand zu Rand
///   läuft: dort gibt es kein Außen, in das ein Ring passen könnte, und zwei
///   Pixel Rand je Zeile zerstörten den Dichte-Rhythmus.
class HFocusRing extends StatelessWidget {
  /// Umgibt [child] mit einem Ring, sobald [visible] gilt, und hält den Platz
  /// dafür immer frei.
  const HFocusRing({
    required this.visible,
    required this.child,
    this.radius,
    this.over,
    super.key,
  }) : _reserve = true;

  /// Zeichnet den Ring auf die Kante von [child], ohne Platz zu reservieren.
  const HFocusRing.inline({
    required this.visible,
    required this.child,
    this.radius,
    super.key,
  }) : over = null,
       _reserve = false;

  /// Die Breite des Rings. Zwei Pixel, wie `docs/UX.md` 6 sie nennt.
  ///
  /// Der Wert steht in [HFocusRingMetrics], weil der Ring der Bibliothek
  /// (`FocusOutline`, gesetzt in `HTheme`) dieselben zwei Pixel braucht.
  static const double width = HFocusRingMetrics.width;

  /// Der Abstand zwischen dem Control und dem Ring.
  ///
  /// Zwei Pixel Fläche, damit der Ring nicht auf der Füllung des Controls
  /// liegt. Gilt nur für [HFocusRing.new] und nur über einer Füllung, gegen
  /// die der Ring sonst verschwände; [HFocusRing.inline] hat kein Außen, in
  /// das ein Abstand passte.
  static const double gap = HFocusRingMetrics.gap;

  /// Ob ein Ring in [ring] neben der Füllung [fill] einen [gap] braucht.
  ///
  /// Nur eine deckende Füllung ist der Nachbar des Rings: eine Tönung und
  /// eine Hover-Füllung lassen die Fläche darunter durchscheinen und kommen
  /// dem Akzent nie nahe genug.
  static bool needsGap(Color? fill, Color ring) =>
      fill != null &&
      fill.a == 1.0 &&
      HColorDerivation.contrast(ring, fill) < HColorDerivation.areaMinContrast;

  /// Wie viel [HFocusRing.new] auf jeder Seite freihält: der Ring, und über
  /// einer Füllung wie [fill] zusätzlich der [gap].
  static double reservedFor(Color? fill, Color ring) =>
      needsGap(fill, ring) ? width + gap : width;

  /// Ob das Control gerade den Tastaturfokus hat.
  final bool visible;

  /// Der Eckenradius des Controls; null heißt eckig.
  final double? radius;

  /// Die deckende Füllung des Controls, an dessen Kante der Ring läge.
  ///
  /// Null für alles, was keine hat. Siehe [needsGap].
  final Color? over;

  /// Das Control.
  final Widget child;

  final bool _reserve;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final double inner = radius ?? 0;
    final double reserved = reservedFor(over, tokens.colors.accent);
    final Widget painted = CustomPaint(
      foregroundPainter: visible
          ? _HFocusRingPainter(
              color: tokens.colors.accent,
              radius: _reserve ? inner + reserved : inner,
            )
          : null,
      child: _reserve
          ? Padding(padding: EdgeInsets.all(reserved), child: child)
          : child,
    );
    return painted;
  }
}

class _HFocusRingPainter extends CustomPainter {
  const _HFocusRingPainter({required this.color, required this.radius});

  final Color color;
  final double radius;

  @override
  void paint(Canvas canvas, Size size) {
    // Die halbe Strichbreite liegt auf dem Rechteck, also wird das Rechteck um
    // eine halbe Strichbreite eingerückt und der Ring liegt bündig auf der
    // äußeren Kante.
    final double inset = HFocusRing.width / 2;
    final RRect ring = RRect.fromRectAndRadius(
      Rect.fromLTWH(
        inset,
        inset,
        size.width - inset * 2,
        size.height - inset * 2,
      ),
      Radius.circular(radius),
    );
    canvas.drawRRect(
      ring,
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = HFocusRing.width
        ..color = color,
    );
  }

  @override
  bool shouldRepaint(_HFocusRingPainter oldDelegate) =>
      oldDelegate.color != color || oldDelegate.radius != radius;
}
