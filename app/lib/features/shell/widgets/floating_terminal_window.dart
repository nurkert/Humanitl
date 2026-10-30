/// The floating terminal's window itself: title bar, terminal and grip
/// (HUM-300).
///
/// Where the window stands, when it steps back and when it closes is decided
/// by its host, `FloatingTerminalLayer` in `floating_terminal.dart`; this file
/// only draws the window and reports what the hand does with it.
library;

import 'package:flutter/gestures.dart' show DragStartBehavior;
import 'package:flutter/widgets.dart';

import '../../../core/ui/hover_label.dart';
import '../../../core/ui/shell_glyph.dart';
import '../../../core/ui/terminal_float.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../../sandbox/widgets/sandbox_terminal.dart';

/// The height of the title bar of the floating window.
const double floatingTerminalTitleHeight = 28;

/// The edge of the square in the inner corner that resizes the window.
const double floatingTerminalGripSize = 14;

/// What the header of the window (title bar and the banner under it) leaves
/// free for the grip at [anchor] (HUM-352): the width of the grip on the
/// side where it sits in the top row, nothing while it sits at the bottom.
EdgeInsets floatingTerminalHeaderInset(FloatAnchor anchor) {
  if (anchor.gripY != -1) {
    return EdgeInsets.zero;
  }
  return anchor.gripX == -1
      ? const EdgeInsets.only(left: floatingTerminalGripSize)
      : const EdgeInsets.only(right: floatingTerminalGripSize);
}

/// The window: a title bar with the buttons, the terminal below, and the grip
/// in the corner across from the anchor.
class FloatingTerminalWindow extends StatelessWidget {
  /// Creates the window held to [anchor].
  const FloatingTerminalWindow({
    required this.anchor,
    required this.onDragStart,
    required this.onDragUpdate,
    required this.onDragEnd,
    required this.onResizeStart,
    required this.onResizeUpdate,
    required this.onResizeEnd,
    required this.onBack,
    required this.onClose,
    super.key,
  });

  /// The anchor the window holds to; the grip sits across from it
  /// ([FloatAnchor.gripX], [FloatAnchor.gripY]).
  final FloatAnchor anchor;

  /// The title bar was taken by the hand.
  final VoidCallback onDragStart;

  /// The title bar moved by the given distance.
  final ValueChanged<Offset> onDragUpdate;

  /// The title bar was let go with the given velocity in logical pixels per
  /// second, or the drag was cancelled ([Offset.zero]).
  final ValueChanged<Offset> onDragEnd;

  /// The grip was taken by the hand.
  final VoidCallback onResizeStart;

  /// The grip moved by the given distance.
  final ValueChanged<Offset> onResizeUpdate;

  /// The grip was let go, or the drag was cancelled.
  final VoidCallback onResizeEnd;

  /// "Back to the sandbox page".
  final VoidCallback onBack;

  /// "Close the floating terminal".
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // Der Griff sitzt in der Ecke des Fensters, gegenüber dem Anker. Steht
    // er oben, liegt er in der Titelleiste; Titelleiste und Banner rücken
    // dann auf seiner Seite um seine Breite ein, sodass weder Titel noch
    // Knöpfe noch der Hinweis unter ihm liegen (HUM-352). Früher begann er
    // unter der Titelleiste und lag so auf dem Hinweis.
    final EdgeInsets headerInset = floatingTerminalHeaderInset(anchor);
    // Kein Schatten (`docs/DESIGN.md`, Antireferenz 5): Die kräftige Linie
    // und die Fläche `bg1` heben das Fenster von der Seite darunter ab.
    return Semantics(
      key: const Key('shell-terminal-floating'),
      container: true,
      label: l10n.shellFloatingTerminalLabel,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: tokens.colors.bg1,
          borderRadius: BorderRadius.circular(tokens.radii.card),
          border: Border.all(color: tokens.colors.lineStrong),
        ),
        child: ClipRRect(
          borderRadius: BorderRadius.circular(tokens.radii.card),
          child: Stack(
            fit: StackFit.expand,
            children: <Widget>[
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  _TitleBar(
                    inset: headerInset,
                    onDragStart: onDragStart,
                    onDragUpdate: onDragUpdate,
                    onDragEnd: onDragEnd,
                    onBack: onBack,
                    onClose: onClose,
                  ),
                  Expanded(
                    child: SandboxTerminal(
                      key: const Key('shell-terminal-floating-view'),
                      catchUp: true,
                      floating: true,
                      headerInset: headerInset,
                    ),
                  ),
                ],
              ),
              Positioned(
                left: anchor.gripX == -1 ? 0 : null,
                right: anchor.gripX == -1 ? null : 0,
                top: anchor.gripY == -1 ? 0 : null,
                bottom: anchor.gripY == -1 ? null : 0,
                width: floatingTerminalGripSize,
                height: floatingTerminalGripSize,
                child: _Grip(
                  anchor: anchor,
                  onStart: onResizeStart,
                  onUpdate: onResizeUpdate,
                  onEnd: onResizeEnd,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The bar the window is dragged by, with "back to the sandbox" and "close".
class _TitleBar extends StatelessWidget {
  const _TitleBar({
    required this.inset,
    required this.onDragStart,
    required this.onDragUpdate,
    required this.onDragEnd,
    required this.onBack,
    required this.onClose,
  });

  /// The room the grip takes on one side of the bar (HUM-352).
  final EdgeInsets inset;
  final VoidCallback onDragStart;
  final ValueChanged<Offset> onDragUpdate;
  final ValueChanged<Offset> onDragEnd;
  final VoidCallback onBack;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return MouseRegion(
      cursor: SystemMouseCursors.move,
      child: GestureDetector(
        key: const Key('shell-terminal-floating-title'),
        behavior: HitTestBehavior.opaque,
        // Vom Druck an gerechnet: Das Fenster folgt dem Zeiger ohne den
        // Sprung um die Schwelle, ab der ein Zug als Zug gilt.
        dragStartBehavior: DragStartBehavior.down,
        onPanStart: (DragStartDetails _) => onDragStart(),
        onPanUpdate: (DragUpdateDetails details) => onDragUpdate(details.delta),
        // Die Geschwindigkeit beim Loslassen entscheidet mit, wohin das
        // Fenster einrastet (HUM-352); ein abgebrochener Zug hat keine.
        onPanEnd: (DragEndDetails details) =>
            onDragEnd(details.velocity.pixelsPerSecond),
        onPanCancel: () => onDragEnd(Offset.zero),
        child: Container(
          height: floatingTerminalTitleHeight,
          color: tokens.colors.bg2,
          padding: EdgeInsets.only(
            left: tokens.spacing.x2 + inset.left,
            right: inset.right,
          ),
          child: Row(
            children: <Widget>[
              Expanded(
                child: Text(
                  l10n.shellFloatingTerminalTitle,
                  style: tokens.typography.ui12.semibold.tinted(
                    tokens.colors.fg0,
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              HoverLabel(
                label: l10n.shellFloatingTerminalBack,
                side: HoverLabelSide.below,
                child: HButton(
                  key: const Key('shell-terminal-floating-back'),
                  variant: HButtonVariant.ghost,
                  size: HButtonSize.sm,
                  semanticsLabel: l10n.shellFloatingTerminalBack,
                  onPressed: onBack,
                  child: ShellGlyphIcon(
                    ShellGlyph.sandbox,
                    size: 14,
                    color: tokens.colors.fg1,
                  ),
                ),
              ),
              HoverLabel(
                label: l10n.shellFloatingTerminalClose,
                side: HoverLabelSide.below,
                child: HIconButton(
                  key: const Key('shell-terminal-floating-close'),
                  glyph: HGlyph.close,
                  semanticsLabel: l10n.shellFloatingTerminalClose,
                  onPressed: onClose,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The square in the inner corner that resizes the window.
class _Grip extends StatelessWidget {
  const _Grip({
    required this.anchor,
    required this.onStart,
    required this.onUpdate,
    required this.onEnd,
  });

  final FloatAnchor anchor;
  final VoidCallback onStart;
  final ValueChanged<Offset> onUpdate;
  final VoidCallback onEnd;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    // Der Griff sitzt gegenüber dem Anker: oben links oder unten rechts zieht
    // schräg nach links oben, die anderen beiden nach rechts oben.
    final bool falling = anchor.gripX == anchor.gripY;
    return MouseRegion(
      cursor: falling
          ? SystemMouseCursors.resizeUpLeftDownRight
          : SystemMouseCursors.resizeUpRightDownLeft,
      child: GestureDetector(
        key: const Key('shell-terminal-floating-grip'),
        behavior: HitTestBehavior.opaque,
        dragStartBehavior: DragStartBehavior.down,
        onPanStart: (DragStartDetails _) => onStart(),
        onPanUpdate: (DragUpdateDetails details) => onUpdate(details.delta),
        onPanEnd: (DragEndDetails _) => onEnd(),
        onPanCancel: onEnd,
        child: CustomPaint(
          painter: _GripPainter(
            color: tokens.colors.fg2,
            // Die Linien liegen in der Ecke des Fensters, in der der Griff
            // sitzt, und laufen quer zu ihr.
            towardsRight: anchor.gripX == 1,
            towardsBottom: anchor.gripY == 1,
          ),
        ),
      ),
    );
  }
}

/// Two short diagonal strokes in the corner of the grip that is the corner of
/// the window.
class _GripPainter extends CustomPainter {
  const _GripPainter({
    required this.color,
    required this.towardsRight,
    required this.towardsBottom,
  });

  final Color color;
  final bool towardsRight;
  final bool towardsBottom;

  @override
  void paint(Canvas canvas, Size size) {
    final Paint stroke = Paint()
      ..color = color
      ..strokeWidth = 1
      ..strokeCap = StrokeCap.round;
    final double w = size.width;
    final double h = size.height;
    Offset at(double x, double y) =>
        Offset(towardsRight ? x : w - x, towardsBottom ? y : h - y);
    for (final double d in <double>[4, 8]) {
      canvas.drawLine(at(w - 2, h - 2 - d), at(w - 2 - d, h - 2), stroke);
    }
  }

  @override
  bool shouldRepaint(_GripPainter oldDelegate) =>
      oldDelegate.color != color ||
      oldDelegate.towardsRight != towardsRight ||
      oldDelegate.towardsBottom != towardsBottom;
}
