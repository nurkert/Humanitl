import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/colors.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';

/// A small tinted label: 11/500, radius 2, ten percent area tint.
///
/// The chip itself is 18 px tall, but the widget always reserves
/// [HSize.hitMin], so a badge is never a hit target below the design minimum.
class HBadge extends StatelessWidget {
  /// Creates a badge showing [text].
  const HBadge({
    required this.text,
    this.color,
    this.mono = false,
    this.onTap,
    this.semanticsLabel,
    super.key,
  });

  /// The label. Already localised by the caller; this package holds no strings.
  final String text;

  /// Tint and text colour; the secondary text colour when null.
  final Color? color;

  /// Uses the monospace family, for protocol tokens.
  final bool mono;

  /// Makes the badge tappable over its full [HSize.hitMin] height.
  final VoidCallback? onTap;

  /// Screen-reader label; [text] is used when null.
  final String? semanticsLabel;

  /// Height of the visible chip, independent of the hit target.
  static const double chipHeight = 18;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color resolved = color ?? tokens.colors.fg1;
    final TextStyle style =
        (mono ? tokens.typography.mono11 : tokens.typography.ui11).medium
            .tinted(resolved);
    // No `alignment:` on the container and `widthFactor: 1` on every Center:
    // a badge shrink-wraps its label. A container with an alignment expands to
    // the incoming constraints, which turns every badge in a column into a
    // full width bar.
    final Widget chip = Container(
      height: chipHeight,
      padding: const EdgeInsets.symmetric(horizontal: HSpace.x2),
      decoration: BoxDecoration(
        color: HColorDerivation.tint(resolved),
        borderRadius: HRadius.badgeRadius,
      ),
      child: Center(
        widthFactor: 1,
        child: Text(
          text,
          style: style,
          maxLines: 1,
          overflow: TextOverflow.clip,
        ),
      ),
    );
    final Widget sized = SizedBox(
      height: HSize.hitMin,
      child: Center(widthFactor: 1, child: chip),
    );
    final Widget labelled = Semantics(
      label: semanticsLabel ?? text,
      button: onTap != null,
      excludeSemantics: true,
      child: sized,
    );
    if (onTap == null) {
      return labelled;
    }
    return GestureDetector(
      onTap: onTap,
      behavior: HitTestBehavior.opaque,
      child: labelled,
    );
  }
}
