import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/flow_state.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import 'h_glyph.dart';

/// A glyph that is also a hit target of [HSize.hitMin].
///
/// Used for the close affordance of [HSheet] and wherever a label would be
/// noise. It is not a [HButton] variant on purpose: a button without a label is
/// a different thing, and giving it its own type keeps the button honest.
class HIconButton extends StatefulWidget {
  /// Creates an icon button showing [glyph].
  const HIconButton({
    required this.glyph,
    required this.onPressed,
    required this.semanticsLabel,
    this.color,
    this.size = HSize.glyph,
    super.key,
  });

  /// The glyph to draw.
  final HGlyph glyph;

  /// Invoked on tap. Null disables the button.
  final VoidCallback? onPressed;

  /// Screen-reader label. Required: an unlabelled glyph is unusable.
  final String semanticsLabel;

  /// Stroke colour; the secondary text colour when null.
  final Color? color;

  /// Edge length of the glyph itself, not of the hit target.
  final double size;

  @override
  State<HIconButton> createState() => _HIconButtonState();
}

class _HIconButtonState extends State<HIconButton> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final bool enabled = widget.onPressed != null;
    final Color color =
        widget.color ?? (_hovered ? tokens.colors.fg0 : tokens.colors.fg1);
    return Semantics(
      button: true,
      enabled: enabled,
      label: widget.semanticsLabel,
      child: MouseRegion(
        onEnter: (PointerEnterEvent _) => setState(() => _hovered = true),
        onExit: (PointerExitEvent _) => setState(() => _hovered = false),
        cursor: enabled ? SystemMouseCursors.click : MouseCursor.defer,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onPressed,
          child: SizedBox.square(
            dimension: HSize.hitMin,
            child: Center(
              child: Opacity(
                opacity: enabled ? 1 : 0.45,
                child: HGlyphIcon(
                  widget.glyph,
                  size: widget.size,
                  color: color,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
