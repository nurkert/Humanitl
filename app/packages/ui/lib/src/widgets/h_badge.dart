import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/colors.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';
import 'h_focus_ring.dart';

/// A small tinted label: 11/500, radius 2, ten percent area tint.
///
/// The chip itself is 18 px tall, but the widget always reserves
/// [HSize.hitMin], so a badge is never a hit target below the design minimum.
///
/// [chipHeight] und die reservierte Höhe sind Mindesthöhen und keine festen
/// Höhen: bei `TextScaler.linear(2.0)` ist die Zeile höher als der Chip, und
/// eine feste Höhe schluckte den Überlauf still (`docs/UX.md` 6 und 9,
/// Punkt 18).
///
/// Fläche und Beschriftung werden getrennt geführt. [color] ist die Fläche —
/// zehn Prozent Tönung —, [textColor] das Wort darauf. Eine Zustands- oder
/// Methodenfarbe ist auf 3:1 geklemmt und trägt damit keinen Text; die
/// Textvariante derselben Farbe erreicht 4,5:1, auch auf ihrer eigenen Tönung
/// (`docs/UX.md` 6 und 9, Punkt 5).
class HBadge extends StatefulWidget {
  /// Creates a badge showing [text].
  const HBadge({
    required this.text,
    this.color,
    this.textColor,
    this.background,
    this.mono = false,
    this.onTap,
    this.semanticsLabel,
    this.focusNode,
    super.key,
  });

  /// The label. Already localised by the caller; this package holds no strings.
  final String text;

  /// Die Fläche: der Ton, aus dem die Tönung gemacht wird, und die
  /// Rückfallfarbe der Beschriftung. Der Sekundärtext, wenn null.
  final Color? color;

  /// Die Farbe der Beschriftung.
  ///
  /// Getrennt geführt, weil eine Fläche 3:1 erreichen muss und ein Wort
  /// 4,5:1. Null heißt: die Textvariante von [color], sofern das eine
  /// Zustandsfarbe ist (`HTokens.stateTextOf`), sonst [color] selbst.
  final Color? textColor;

  /// Die Fläche des Chips, wenn sie nicht die Tönung von [color] sein soll.
  ///
  /// Der neutrale Method-Badge einer Liste steht so auf `bg2`: in einer Liste
  /// liest das Auge ein rötliches Badge neben einer roten Rail als zwei
  /// Blöcke, nicht als ein Verb und einen Zustand (`docs/UX.md` 3.3, Regel 4).
  final Color? background;

  /// Uses the monospace family, for protocol tokens.
  final bool mono;

  /// Makes the badge tappable over its full [HSize.hitMin] height.
  final VoidCallback? onTap;

  /// Screen-reader label; [text] is used when null.
  final String? semanticsLabel;

  /// Ein von außen gehaltener Fokusknoten, für einen Badge mit [onTap].
  final FocusNode? focusNode;

  /// Mindesthöhe des sichtbaren Chips, unabhängig vom Hit-Target.
  static const double chipHeight = 18;

  @override
  State<HBadge> createState() => _HBadgeState();
}

class _HBadgeState extends State<HBadge> {
  bool _focused = false;

  void _setFocused(bool value) {
    if (_focused != value) {
      setState(() => _focused = value);
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final String text = widget.text;
    final VoidCallback? onTap = widget.onTap;
    final Color resolved = widget.color ?? tokens.colors.fg1;
    // Ohne ausdrückliche Beschriftungsfarbe: die Textvariante der Fläche,
    // sofern es eine Zustandsfarbe ist. Eine Zustandsfarbe ist auf 3:1
    // geklemmt und trägt damit kein Wort (`docs/UX.md` 6).
    final Color label = widget.textColor ?? tokens.stateTextOf(resolved);
    final TextStyle style =
        (widget.mono ? tokens.typography.mono11 : tokens.typography.ui11).medium
            .tinted(label);
    // No `alignment:` on the container and `widthFactor: 1` on every Center:
    // a badge shrink-wraps its label. A container with an alignment expands to
    // the incoming constraints, which turns every badge in a column into a
    // full width bar.
    final Widget chip = Container(
      constraints: const BoxConstraints(minHeight: HBadge.chipHeight),
      padding: const EdgeInsets.symmetric(horizontal: HSpace.x2),
      decoration: BoxDecoration(
        color: widget.background ?? HColorDerivation.tint(resolved),
        borderRadius: HRadius.badgeRadius,
      ),
      child: Center(
        widthFactor: 1,
        heightFactor: 1,
        child: Text(
          text,
          style: style,
          maxLines: 1,
          overflow: TextOverflow.clip,
        ),
      ),
    );
    final Widget sized = ConstrainedBox(
      constraints: const BoxConstraints(minHeight: HSize.hitMin),
      // heightFactor: 1, sonst dehnt sich der Badge auf die volle verfügbare
      // Höhe: die reservierte Höhe ist eine Untergrenze, keine Höhe.
      child: Center(widthFactor: 1, heightFactor: 1, child: chip),
    );
    final Widget labelled = Semantics(
      label: widget.semanticsLabel ?? text,
      button: onTap != null,
      excludeSemantics: true,
      child: sized,
    );
    if (onTap == null) {
      return labelled;
    }
    // Ein Badge, den man anfassen kann, ist ein Control und damit ein
    // Fokusstopp (`docs/UX.md` 5.1 und 9, Punkt 17).
    return FocusableActionDetector(
      focusNode: widget.focusNode,
      mouseCursor: SystemMouseCursors.click,
      onFocusChange: _setFocused,
      actions: <Type, Action<Intent>>{
        ActivateIntent: CallbackAction<ActivateIntent>(
          onInvoke: (ActivateIntent intent) {
            onTap();
            return null;
          },
        ),
      },
      child: HFocusRing(
        visible: _focused,
        radius: tokens.radii.badge,
        child: GestureDetector(
          onTap: onTap,
          behavior: HitTestBehavior.opaque,
          child: labelled,
        ),
      ),
    );
  }
}
