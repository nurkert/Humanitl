import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/tokens.dart';
import 'h_badge.dart';

/// The HTTP method of a request, as an uppercase monospace badge.
///
/// Method hues are deliberately *not* state colours: a `DELETE` is not a block.
/// `DELETE` therefore borrows the blocked hue at seventy percent, which reads as
/// a warning without claiming a decision was made. The table itself lives in
/// [HMethodColors]; this widget only reads it.
class HMethodBadge extends StatelessWidget {
  /// Creates a badge for [method].
  const HMethodBadge({
    required this.method,
    this.neutral = false,
    this.semanticsLabel,
    super.key,
  });

  /// The HTTP method, in any case; it is displayed uppercase.
  final String method;

  /// Die neutrale Variante für Listen: `fg1` auf `bg2`, kein Ton.
  ///
  /// Die Methoden-Hues borgen sich Zustandsfarben. In einer Liste steht ein
  /// rötliches `DELETE` neben einer roten Rail, und das Auge liest zwei
  /// Blöcke statt eines Verbs und eines Zustands. Ton bekommt nur das eine
  /// Badge im Kartenkopf (`docs/UX.md` 3.3, Regel 4, und 9, Punkt 13).
  final bool neutral;

  /// Screen-reader label; the uppercase method when null.
  final String? semanticsLabel;

  /// The hue of [method] in [tokens], as [HMethodColors.of] resolves it.
  static Color colorFor(String method, HTokens tokens) =>
      tokens.method.of(method);

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final String upper = method.toUpperCase();
    if (neutral) {
      return HBadge(
        text: upper,
        color: tokens.colors.fg1,
        background: tokens.colors.bg2,
        mono: true,
        semanticsLabel: semanticsLabel ?? upper,
      );
    }
    return HBadge(
      text: upper,
      color: colorFor(method, tokens),
      // Die Fläche trägt den Ton, das Kürzel die Textvariante desselben Tons:
      // auf der eigenen Tönung misst `DELETE` sonst 2,65:1 (`docs/UX.md` 6).
      textColor: tokens.methodTextColor(method),
      mono: true,
      semanticsLabel: semanticsLabel ?? upper,
    );
  }
}
