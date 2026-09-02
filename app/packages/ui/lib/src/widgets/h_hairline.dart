import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';

/// A one pixel separator. The design uses hairlines where others use shadows.
class HHairline extends StatelessWidget {
  /// Creates a hairline.
  const HHairline({
    this.vertical = false,
    this.color,
    this.length,
    this.strong = false,
    super.key,
  });

  /// True for a vertical rule, false for a horizontal one.
  final bool vertical;

  /// Overrides the line colour of the theme.
  final Color? color;

  /// Length along the line. Null fills the parent, which therefore has to
  /// constrain that axis.
  final double? length;

  /// Uses the emphasised line colour.
  final bool strong;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color resolved =
        color ?? (strong ? tokens.colors.lineStrong : tokens.colors.line);
    return SizedBox(
      width: vertical ? HSize.hairline : (length ?? double.infinity),
      height: vertical ? (length ?? double.infinity) : HSize.hairline,
      child: ColoredBox(color: resolved),
    );
  }
}
