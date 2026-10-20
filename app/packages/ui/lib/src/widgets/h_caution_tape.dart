import 'package:flutter/widgets.dart';

import '../tokens/colors.dart';
import '../tokens/motion.dart';
import '../tokens/spacing.dart';

/// Das Absperrband: diagonale Streifen in Markengelb und Schwarz, die
/// gleichmäßig seitwärts laufen (HUM-260).
///
/// Humanitl ist die Absperrung, an der der Verkehr hält, und das Band ist ihr
/// Zeichen. Es ist **kein Zustand und kein Control**: es trägt weder eine
/// Zustandsfarbe noch den Akzent in seiner Bedeutung „hier anfassen", sondern
/// die Farben der Marke, in beiden Themes dieselben, wie ein echtes Band. Wo
/// es stehen darf und wo nie, regelt `docs/UX.md` 3.6.
///
/// **Günstig gezeichnet.** Ein `AnimationController` treibt einen
/// [CustomPainter] direkt über `repaint`; das Widget baut beim Laufen nie neu.
/// Eine [RepaintBoundary] hält das Neuzeichnen auf die vier Pixel des Bands,
/// damit nicht die Warteschlange darunter in jedem Frame mitgezeichnet wird.
/// Der Ticker kommt aus [SingleTickerProviderStateMixin] und schweigt, wenn
/// ein [TickerMode] darüber ihn abschaltet — eine Sektion im `IndexedStack`,
/// die gerade nicht sichtbar ist, zeichnet also nichts.
///
/// **Nahtlos.** Der Controller läuft linear von 0 bis 1 und beginnt dann von
/// vorn; die Verschiebung ist `Wert × Periode`, und eine Verschiebung um eine
/// volle Periode zeichnet dasselbe Bild wie keine. Es gibt also keinen Sprung
/// am Ende eines Durchlaufs.
///
/// **Reduzierte Bewegung** (`docs/UX.md` 2.10): Unter [HReducedMotion] steht
/// das Band still, die Streifen bleiben. Der Weg fällt weg, die Aussage
/// bleibt.
class HCautionTape extends StatefulWidget {
  /// Ein Band der Höhe [height], so breit wie sein Platz.
  const HCautionTape({this.height = HSize.cautionTape, super.key});

  /// Die Höhe des Bands in logischen Pixeln.
  final double height;

  /// Hält jedes Band an, nur für Tests.
  ///
  /// Ein Band, das endlos läuft, plant endlos Frames, und
  /// `WidgetTester.pumpAndSettle` wartete dann vergeblich. Die Tests der
  /// Anwendung und die Goldens setzen den Schalter deshalb in
  /// `app/test/flutter_test_config.dart`; dort steht das Band als Standbild
  /// bei Phase null. Die Tests des Bands selbst lassen ihn aus. Im Programm
  /// bleibt er `false`.
  static bool debugFreeze = false;

  @override
  State<HCautionTape> createState() => _HCautionTapeState();
}

class _HCautionTapeState extends State<HCautionTape>
    with SingleTickerProviderStateMixin {
  late final AnimationController _phase = AnimationController(
    vsync: this,
    duration: HMotion.cautionTape,
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _sync();
  }

  @override
  void didUpdateWidget(HCautionTape oldWidget) {
    super.didUpdateWidget(oldWidget);
    _sync();
  }

  void _sync() {
    final bool still = HCautionTape.debugFreeze || HReducedMotion.of(context);
    if (still) {
      if (_phase.isAnimating) {
        _phase.stop();
      }
      // Stillstand heißt eine feste Phase, nicht die zufällige, bei der die
      // Einstellung umgeschaltet wurde: ein Standbild sieht immer gleich aus.
      _phase.value = 0;
    } else if (!_phase.isAnimating) {
      _phase.repeat();
    }
  }

  @override
  void dispose() {
    _phase.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ExcludeSemantics(
    child: RepaintBoundary(
      child: SizedBox(
        height: widget.height,
        width: double.infinity,
        child: CustomPaint(painter: HCautionTapePainter(phase: _phase)),
      ),
    ),
  );
}

/// Zeichnet die Streifen des [HCautionTape] bei der Phase [phase].
///
/// Öffentlich, damit ein Test die Geometrie nachmessen kann, ohne Pixel zu
/// lesen: [stripeStarts] liefert die linken unteren Ecken der gelben Streifen.
class HCautionTapePainter extends CustomPainter {
  /// Ein Maler, der bei jeder Änderung von [phase] neu zeichnet.
  HCautionTapePainter({required this.phase}) : super(repaint: phase);

  /// Die Phase von 0 bis 1; 1 zeichnet dasselbe wie 0.
  final Animation<double> phase;

  /// Die gelbe Farbe des Bands: das Gelb der Marke, in beiden Themes.
  static const Color yellow = HColors.accent;

  /// Die schwarze Farbe des Bands, dieselbe wie der Text auf dem Akzent.
  static const Color black = HColors.onAccent;

  /// Die linken unteren Ecken der gelben Streifen bei [value] auf einem Band
  /// der Größe [size], von links nach rechts.
  ///
  /// Der erste Streifen beginnt links außerhalb, sodass auch die linke Kante
  /// immer bedeckt ist; der letzte beginnt vor der rechten Kante.
  static List<double> stripeStarts(double value, Size size) {
    const double period = HMotion.cautionTapePeriod;
    final double shift = (value % 1) * period;
    final List<double> starts = <double>[];
    for (double x = shift - period - size.height; x < size.width; x += period) {
      starts.add(x);
    }
    return starts;
  }

  @override
  void paint(Canvas canvas, Size size) {
    final Rect bounds = Offset.zero & size;
    canvas
      ..save()
      ..clipRect(bounds)
      ..drawRect(bounds, Paint()..color = black);
    const double half = HMotion.cautionTapePeriod / 2;
    final double h = size.height;
    final Path stripes = Path();
    // Streifen unter 45 Grad: unten links beginnend, oben um die Höhe nach
    // rechts versetzt.
    for (final double x in stripeStarts(phase.value, size)) {
      stripes
        ..moveTo(x, h)
        ..lineTo(x + h, 0)
        ..lineTo(x + h + half, 0)
        ..lineTo(x + half, h)
        ..close();
    }
    canvas
      ..drawPath(stripes, Paint()..color = yellow)
      ..restore();
  }

  @override
  bool shouldRepaint(HCautionTapePainter oldDelegate) =>
      oldDelegate.phase != phase;
}
