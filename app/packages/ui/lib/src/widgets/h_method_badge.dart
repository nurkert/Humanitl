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
  const HMethodBadge({required this.method, this.semanticsLabel, super.key});

  /// The HTTP method, in any case; it is displayed uppercase.
  final String method;

  /// Screen-reader label; the uppercase method when null.
  final String? semanticsLabel;

  /// The hue of [method] in [tokens], as [HMethodColors.of] resolves it.
  static Color colorFor(String method, HTokens tokens) =>
      tokens.method.of(method);

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final String upper = method.toUpperCase();
    return HBadge(
      text: upper,
      color: colorFor(method, tokens),
      mono: true,
      semanticsLabel: semanticsLabel ?? upper,
    );
  }
}
