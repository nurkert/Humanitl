import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../theme/shadcn_theme.dart';
import '../tokens/flow_state.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import 'h_control.dart';
import 'h_glyph.dart';

/// A glyph that is also a hit target of [HSize.hitMin].
///
/// Used for the close affordance of [HSheet] and wherever a label would be
/// noise. It is not a [HButton] variant on purpose: a button without a label is
/// a different thing, and giving it its own type keeps the button honest.
///
/// Es steht wie jedes Control des Pakets über [HControl] auf `Clickable` aus
/// `shadcn_flutter`, in der stillen Rolle: keine Fläche bei Ruhe, `bg2` unter
/// dem Zeiger, `bg3` beim Druck. Die Füllung ist neu — vorher wechselte nur
/// die Strichfarbe, und ein Glyph ohne Fläche macht den Druck kaum sichtbar.
///
/// Fokussierbar wie jedes andere Control: `docs/UX.md` 5.1 verlangt Parität
/// zwischen Zeiger und Tastatur, und ein Glyph, das nur die Maus erreicht, ist
/// eine Handlung ohne Taste (`docs/UX.md` 9, Punkt 17). Der Fokus zeigt sich
/// als `HFocusRing` außerhalb des Ziels, in einem Frame.
class HIconButton extends StatelessWidget {
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
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final bool enabled = onPressed != null;
    return Semantics(
      button: true,
      enabled: enabled,
      label: semanticsLabel,
      child: HControl(
        onPressed: onPressed,
        focusNode: focusNode,
        autofocus: autofocus,
        radius: tokens.radii.control,
        fill: (HTokens tokens, Set<WidgetState> states) =>
            HShadcnButtonStyle.fillOf(tokens, HShadcnButtonRole.ghost, states),
        style: (HTokens tokens, Color fill) => HShadcnButtonStyle.of(
          tokens,
          HShadcnButtonRole.ghost,
          padding: EdgeInsets.zero,
          fill: fill,
        ),
        builder: (BuildContext context, Set<WidgetState> states, Color fill) {
          // Deaktiviert heißt sichtbar deaktiviert: `fg2` ist die Stufe, die
          // `docs/UX.md` 6 dafür freihält. Unter Zeiger oder Fokus tritt das
          // Glyph auf `fg0` vor.
          final Color stroke = states.contains(WidgetState.disabled)
              ? tokens.colors.fg2
              : color ??
                    (states.contains(WidgetState.hovered) ||
                            states.contains(WidgetState.focused)
                        ? tokens.colors.fg0
                        : tokens.colors.fg1);
          return SizedBox.square(
            dimension: HSize.hitMin,
            child: Center(
              child: HGlyphIcon(glyph, size: size, color: stroke),
            ),
          );
        },
      ),
    );
  }
}
