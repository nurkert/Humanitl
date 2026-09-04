/// A two pixel accent ring outside a control, painted in one frame.
///
/// Das Designsystem hat seinen eigenen Ring: `HFocusRing` seit HUM-035, und
/// `HButton` trägt ihn. Dieses Widget ist damit dasselbe zweimal
/// (`docs/UX.md` 9, Punkt 31) und bleibt nur, weil fünf Controls der
/// Aktionsleiste und der History es benutzen — es fällt weg, wenn die
/// Handoff-Widgets nach `packages/ui` ziehen, nicht vorher und nicht in einem
/// Commit, der etwas anderes tut. Bis dahin liest es seine Geometrie aus
/// `HFocusRing`, damit die beiden Ringe nicht auseinanderlaufen.
library;

import 'package:flutter/widgets.dart';

import 'ui.dart';

/// Width of the ring. Two pixels, outside the control's own border.
const double focusRingWidth = HFocusRing.width;

/// The room the ring reserves on each side of a control filled with [fill].
///
/// Über einer deckenden Füllung, gegen die der Akzent verschwände, kommt der
/// Abstand von `HFocusRing.gap` dazu; ohne ihn läge der Ring auf der Füllung
/// und stünde bei 1,00:1 gegen sie (`docs/UX.md` 6). Dieselbe Rechnung wie im
/// Designsystem, damit die beiden Ringe nicht auseinanderlaufen.
double focusRingReserved(Color? fill, Color ring) =>
    HFocusRing.reservedFor(fill, ring);

/// Draws an accent ring around [child] while [visible].
///
/// Callers pass `FocusableActionDetector.onFocusChange`, not
/// `onShowFocusHighlight`: the highlight callback lands one frame late and
/// only in keyboard mode, and both are wrong here. A ring that arrives a frame
/// after the focus reads as input lag, and on this screen the ring is the only
/// sign that `Enter` now belongs to the control instead of to the queue
/// (`docs/UX.md` 5.2 and 6).
///
/// The room for the ring is reserved whether it shows or not, so nothing moves
/// when the focus arrives; the ring lies outside the control's own border and
/// never replaces it, and over an opaque [fill] it keeps `HFocusRing.gap` of
/// surface between itself and the control. There is no animation: a focus ring
/// appears in the frame the focus does.
class FocusRing extends StatelessWidget {
  /// Wraps [child].
  const FocusRing({
    required this.visible,
    required this.radius,
    required this.child,
    this.fill,
    super.key,
  });

  /// Whether the control has the keyboard focus.
  final bool visible;

  /// Corner radius of the control the ring goes around.
  final double radius;

  /// The opaque fill of the control, when it has one. See [focusRingReserved].
  final Color? fill;

  /// The control.
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final double reserved = focusRingReserved(fill, tokens.colors.accent);
    return CustomPaint(
      foregroundPainter: visible
          ? _RingPainter(color: tokens.colors.accent, radius: radius + reserved)
          : null,
      child: Padding(padding: EdgeInsets.all(reserved), child: child),
    );
  }
}

class _RingPainter extends CustomPainter {
  const _RingPainter({required this.color, required this.radius});

  final Color color;
  final double radius;

  @override
  void paint(Canvas canvas, Size size) {
    // Half the stroke lies on the rectangle, so the rectangle is inset by
    // half a stroke and the ring hugs the outer edge of the control.
    final double inset = focusRingWidth / 2;
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
        ..strokeWidth = focusRingWidth
        ..color = color,
    );
  }

  @override
  bool shouldRepaint(_RingPainter old) =>
      old.color != color || old.radius != radius;
}
