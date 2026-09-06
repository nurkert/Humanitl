import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/flow_state.dart';
import '../tokens/motion.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';
import 'h_icon_button.dart';

/// Auf welcher Kante die Spitze sitzt, also wohin das Popover zeigt.
enum HPopoverArrow {
  /// Die Spitze sitzt unten: Das Popover steht **über** dem, worauf es zeigt.
  down,

  /// Die Spitze sitzt oben: Das Popover steht **unter** dem, worauf es zeigt.
  up,
}

/// Ein kleiner Kasten mit einer Spitze, der auf etwas daneben zeigt.
///
/// Gedacht für den einen Satz, der einmal erklärt, wozu ein Control da ist,
/// und danach nie wieder. Er trägt keinen einzigen Text selbst: Überschrift,
/// Satz und die Beschriftung des Schließknopfs kommen fertig übersetzt herein.
///
/// # Drei Eigenschaften, die kein Zufall sind
///
/// - **Er steht neben seinem Ziel, nicht darüber.** Das Widget malt keine
///   Ebene über dem Bildschirm und rechnet keine Koordinaten aus; es ist ein
///   gewöhnliches Kind im Layout, und wer es über die Aktionsleiste setzt,
///   bekommt einen Kasten, der die Leiste **nicht verdecken kann** — nicht
///   weil die Geometrie stimmt, sondern weil er woanders liegt. Eine
///   Erklärung, die die Entscheidung verdeckt, die sie erklärt, ist schlimmer
///   als keine.
/// - **Er nimmt keinen Fokus und bindet keine Taste.** Der Knopf schließt ihn,
///   und `Esc` bindet der Host: Die Taste kommt dort an, wo der Fokus ist, und
///   ein Popover, das ihn an sich nähme, um seine eigene Taste zu bekommen,
///   nähme ihn der Entscheidung weg, die es erklärt (`docs/UX.md` 5.2). Ein
///   `Shortcuts` hier im Widget wäre eine Bindung, die nie feuert, und eine
///   gebundene Taste, die nichts tut, verbietet `docs/UX.md` 5.3. Im Programm
///   bindet `InterceptScreen` sie über `CloseCoachMarkIntent`.
/// - **Er kommt an, er blinkt nicht.** Ein Auftauchen über [HMotion.arrive],
///   [HMotion.arriveOffset] weit von oben herunter; es sagt „hier ist etwas
///   dazugekommen" und sonst nichts (`docs/UX.md` 2.1).
///
/// # Was die Bewegung kostet, und was von ihr unter reduzierter Bewegung übrig
/// bleibt
///
/// Die Strecke läuft über [HReducedMotion.distance] und wird damit null, sobald
/// das System reduzierte Bewegung verlangt; das Ausblenden behält seine volle
/// Dauer und bleibt die Rückmeldung, die sagt, dass der Kasten neu ist. Weniger
/// Weg, nicht weniger Rückmeldung (`docs/UX.md` 2.10). Ist die Strecke null,
/// entfällt die Verschiebungsschicht ganz, statt null Pixel weit zu rechnen.
///
/// Das Ausblenden hängt an einer [FadeTransition] und nicht an einem
/// [Opacity] in einem Builder: die Transition schreibt in ihr Renderobjekt und
/// baut den Kindbaum nicht je Frame neu (`docs/UX.md` 7). Für die Strecke gibt
/// es keine Entsprechung — [SlideTransition] misst ihren Versatz als Bruchteil
/// des Kindes, und hier sind es acht logische Pixel eines Kastens, dessen Höhe
/// erst der Satz darin bestimmt. Der [AnimatedBuilder] bekommt sein Kind
/// deshalb fertig gebaut übergeben; neu entsteht je Frame nur die Verschiebung
/// selbst, genau wie in [SlideTransition]. Ist die Bewegung durch, verlässt
/// die ganze Umhüllung den Baum.
class HPopover extends StatefulWidget {
  /// Baut ein Popover mit [title] über [body].
  const HPopover({
    required this.title,
    required this.body,
    required this.closeLabel,
    required this.onClose,
    this.arrow = HPopoverArrow.down,
    this.arrowInset = 48,
    this.width = 420,
    super.key,
  });

  /// Die Überschrift, fertig übersetzt.
  final String title;

  /// Der Satz, fertig übersetzt.
  final String body;

  /// Beschriftung des Schließknopfs für den Screenreader.
  final String closeLabel;

  /// Wird gerufen, wenn jemand schließt.
  final VoidCallback onClose;

  /// Auf welcher Kante die Spitze sitzt.
  final HPopoverArrow arrow;

  /// Abstand der Spitze vom linken Rand.
  final double arrowInset;

  /// Breite des Kastens.
  final double width;

  /// Breite der Spitze.
  static const double arrowWidth = 14;

  /// Höhe der Spitze.
  static const double arrowHeight = 7;

  @override
  State<HPopover> createState() => _HPopoverState();
}

class _HPopoverState extends State<HPopover>
    with SingleTickerProviderStateMixin {
  /// Die Ankunft.
  ///
  /// [AnimationBehavior.preserve], und das ist hier kein Detail: Meldet die
  /// Plattform `disableAnimations` — und der Linux-Embedder meldet es —, dann
  /// skaliert Flutter jede andere Dauer auf fünf Prozent. Aus den 180 ms des
  /// Ausblendens würden neun, und übrig bliebe ein Kasten, der aufpoppt. Die
  /// Strecke fällt unter reduzierter Bewegung weg, die Rückmeldung nicht
  /// (`docs/UX.md` 2.10, derselbe Grund wie bei `HAnimatedFill`).
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: HMotion.arrive,
    animationBehavior: AnimationBehavior.preserve,
  );

  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _controller,
    curve: HMotion.enter,
  );

  /// Ob die Ankunft vorbei ist und die Umhüllung nichts mehr zu tun hat.
  bool _arrived = false;

  @override
  void initState() {
    super.initState();
    _controller
      ..addStatusListener(_finish)
      ..forward();
  }

  /// Der Animationswrapper verlässt den Baum, sobald er fertig ist
  /// (`docs/UX.md` 7).
  void _finish(AnimationStatus status) {
    if (status == AnimationStatus.completed && mounted && !_arrived) {
      setState(() => _arrived = true);
    }
  }

  @override
  void dispose() {
    _curve.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Widget tip = CustomPaint(
      size: const Size(HPopover.arrowWidth, HPopover.arrowHeight),
      painter: _ArrowPainter(
        color: tokens.colors.bg2,
        line: tokens.colors.line,
        pointsDown: widget.arrow == HPopoverArrow.down,
      ),
    );
    final Widget box = Container(
      width: widget.width,
      padding: EdgeInsets.all(tokens.spacing.x3),
      decoration: BoxDecoration(
        color: tokens.colors.bg2,
        border: Border.all(color: tokens.colors.line, width: HSize.hairline),
        borderRadius: BorderRadius.circular(tokens.radii.panel),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Expanded(
                child: Text(
                  widget.title,
                  style: tokens.typography.ui13.semibold.tinted(
                    tokens.colors.fg0,
                  ),
                ),
              ),
              HIconButton(
                glyph: HGlyph.close,
                onPressed: widget.onClose,
                semanticsLabel: widget.closeLabel,
              ),
            ],
          ),
          SizedBox(height: tokens.spacing.x2),
          Text(
            widget.body,
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
        ],
      ),
    );
    final Widget content = Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        if (widget.arrow == HPopoverArrow.up)
          Padding(
            padding: EdgeInsets.only(left: widget.arrowInset),
            child: tip,
          ),
        box,
        if (widget.arrow == HPopoverArrow.down)
          Padding(
            padding: EdgeInsets.only(left: widget.arrowInset),
            child: tip,
          ),
      ],
    );
    if (_arrived) {
      return content;
    }
    final Widget faded = FadeTransition(opacity: _curve, child: content);
    final double travel = HReducedMotion.distance(
      context,
      HMotion.arriveOffset,
    );
    if (travel == 0) {
      return faded;
    }
    return AnimatedBuilder(
      animation: _curve,
      child: faded,
      builder: (BuildContext context, Widget? child) => Transform.translate(
        offset: Offset(0, -travel * (1 - _curve.value)),
        child: child,
      ),
    );
  }
}

/// Die Spitze: ein Dreieck in der Fläche des Kastens, mit seiner Haarlinie an
/// den beiden schrägen Kanten.
class _ArrowPainter extends CustomPainter {
  const _ArrowPainter({
    required this.color,
    required this.line,
    required this.pointsDown,
  });

  final Color color;
  final Color line;
  final bool pointsDown;

  @override
  void paint(Canvas canvas, Size size) {
    final Path path = Path();
    if (pointsDown) {
      path
        ..moveTo(0, 0)
        ..lineTo(size.width / 2, size.height)
        ..lineTo(size.width, 0);
    } else {
      path
        ..moveTo(0, size.height)
        ..lineTo(size.width / 2, 0)
        ..lineTo(size.width, size.height);
    }
    canvas
      ..drawPath(
        Path.from(path)..close(),
        Paint()
          ..color = color
          ..style = PaintingStyle.fill,
      )
      ..drawPath(
        path,
        Paint()
          ..color = line
          ..style = PaintingStyle.stroke
          ..strokeWidth = HSize.hairline,
      );
  }

  @override
  bool shouldRepaint(_ArrowPainter oldDelegate) =>
      oldDelegate.color != color ||
      oldDelegate.line != line ||
      oldDelegate.pointsDown != pointsDown;
}
