import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/flow_state.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import 'h_focus_ring.dart';
import 'h_glyph.dart';

/// A glyph that is also a hit target of [HSize.hitMin].
///
/// Used for the close affordance of [HSheet] and wherever a label would be
/// noise. It is not a [HButton] variant on purpose: a button without a label is
/// a different thing, and giving it its own type keeps the button honest.
///
/// Fokussierbar wie jedes andere Control: `docs/UX.md` 5.1 verlangt Parität
/// zwischen Zeiger und Tastatur, und ein Glyph, das nur die Maus erreicht, ist
/// eine Handlung ohne Taste (`docs/UX.md` 9, Punkt 17). Der Fokus zeigt sich
/// als [HFocusRing] außerhalb des Ziels, in einem Frame.
class HIconButton extends StatefulWidget {
  /// Creates an icon button showing [glyph].
  const HIconButton({
    required this.glyph,
    required this.onPressed,
    required this.semanticsLabel,
    this.color,
    this.size = HSize.glyph,
    this.autofocus = false,
    this.focusNode,
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

  /// Nimmt den Fokus, sobald das Control zum ersten Mal gebaut wird.
  final bool autofocus;

  /// Ein von außen gehaltener Fokusknoten, damit ein Screen die Reihenfolge
  /// seiner Fokusstopps selbst bestimmen kann.
  final FocusNode? focusNode;

  @override
  State<HIconButton> createState() => _HIconButtonState();
}

class _HIconButtonState extends State<HIconButton> {
  bool _hovered = false;
  bool _focused = false;

  void _setHovered(bool value) {
    if (_hovered != value) {
      setState(() => _hovered = value);
    }
  }

  void _setFocused(bool value) {
    if (_focused != value) {
      setState(() => _focused = value);
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final bool enabled = widget.onPressed != null;
    final Color color =
        widget.color ??
        (_hovered || _focused ? tokens.colors.fg0 : tokens.colors.fg1);
    return Semantics(
      button: true,
      enabled: enabled,
      label: widget.semanticsLabel,
      child: FocusableActionDetector(
        enabled: enabled,
        autofocus: widget.autofocus,
        focusNode: widget.focusNode,
        mouseCursor: enabled ? SystemMouseCursors.click : MouseCursor.defer,
        onShowHoverHighlight: _setHovered,
        onFocusChange: _setFocused,
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (ActivateIntent intent) {
              widget.onPressed?.call();
              return null;
            },
          ),
        },
        child: HFocusRing(
          visible: _focused && enabled,
          radius: tokens.radii.control,
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
      ),
    );
  }
}
