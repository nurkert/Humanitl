import 'package:flutter/widgets.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' as shad;

import '../theme/h_theme.dart';
import '../theme/shadcn_theme.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import 'h_control.dart';

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
/// Ein Badge ist in `shadcn_flutter` ein Button, und hier ebenso: ohne
/// [onTap] steht er als `PrimaryBadge` der Bibliothek — derselbe Rumpf, ohne
/// Fokus und ohne Zeigerweg —, mit [onTap] als [HControl], damit er ein
/// Fokusstopp mit Ring ist (`docs/UX.md` 9, Punkt 17). Den Stil liefert in
/// beiden Fällen [HShadcnButtonStyle.badge].
///
/// Fläche und Beschriftung werden getrennt geführt. [color] ist die Fläche —
/// zehn Prozent Tönung —, [textColor] das Wort darauf. Eine Zustands- oder
/// Methodenfarbe ist auf 3:1 geklemmt und trägt damit keinen Text; die
/// Textvariante derselben Farbe erreicht 4,5:1, auch auf ihrer eigenen Tönung
/// (`docs/UX.md` 6 und 9, Punkt 5).
class HBadge extends StatelessWidget {
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
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color resolved = color ?? tokens.colors.fg1;
    shad.AbstractButtonStyle styleOf(HTokens tokens) =>
        HShadcnButtonStyle.badge(
          tokens,
          resolved,
          background: background,
          textColor: textColor,
          mono: mono,
        );
    // Kein `alignment` am Kasten und `widthFactor: 1` an jedem Center: ein
    // Badge schrumpft auf seine Beschriftung. Ein Kasten mit Ausrichtung
    // dehnt sich auf die einlaufenden Constraints, und damit wird jeder Badge
    // in einer Spalte zu einem Balken über die volle Breite.
    Widget label() => ConstrainedBox(
      constraints: const BoxConstraints(minHeight: HBadge.chipHeight),
      child: Center(
        widthFactor: 1,
        heightFactor: 1,
        child: Text(text, maxLines: 1, overflow: TextOverflow.clip),
      ),
    );

    final Widget chip = onTap == null
        ? HTheme.host(
            context,
            shad.PrimaryBadge(style: styleOf(tokens), child: label()),
          )
        : HControl(
            onPressed: onTap,
            focusNode: focusNode,
            radius: tokens.radii.badge,
            fill: (HTokens tokens, Set<WidgetState> states) =>
                HShadcnButtonStyle.badgeFill(
                  resolved,
                  states,
                  background: background,
                ),
            style: (HTokens tokens, Color fill) => HShadcnButtonStyle.badge(
              tokens,
              resolved,
              background: fill,
              textColor: textColor,
              mono: mono,
            ),
            builder: (
              BuildContext context,
              Set<WidgetState> states,
              Color fill,
            ) => label(),
          );

    return Semantics(
      label: semanticsLabel ?? text,
      button: onTap != null,
      excludeSemantics: true,
      child: ConstrainedBox(
        constraints: const BoxConstraints(minHeight: HSize.hitMin),
        // heightFactor: 1, sonst dehnt sich der Badge auf die volle verfügbare
        // Höhe: die reservierte Höhe ist eine Untergrenze, keine Höhe.
        child: Center(widthFactor: 1, heightFactor: 1, child: chip),
      ),
    );
  }
}
