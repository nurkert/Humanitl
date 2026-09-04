import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../theme/shadcn_theme.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import 'h_control.dart';

/// The four button roles. There is no fifth.
enum HButtonVariant {
  /// The one action a surface wants: filled with the accent.
  primary,

  /// A second, equal action: a tinted surface with a hairline.
  secondary,

  /// A quiet action: no surface until hovered.
  ghost,

  /// A destructive action, in the blocked hue. "Block" is one of them: it is
  /// the decision that stops a request, it is drawn in the state colour of a
  /// blocked flow, and the action bar of the intercept screen asks for exactly
  /// this variant (backlog/sprint-1.md, HUM-020). There is no separate
  /// `destructive` role; this is it.
  danger,
}

/// Die Rolle, unter der [HShadcnButtonStyle] eine Variante kennt.
///
/// Eine Funktion und kein Getter auf [HButtonVariant]: die Rolle und der
/// Stil, den sie in der Bibliothek trägt, sind Sache dieses Pakets. Stünde
/// sie als öffentliches Glied der Aufzählung, führte die öffentliche
/// Schnittstelle einen Typ, den `humanitl_ui.dart` nicht exportiert — und der
/// nächste Schritt wäre ein Feature, das ihn benutzt. Die Zuordnung zu den
/// Varianzen der Bibliothek — `primary`, `secondary`, `ghost`, `destructive` —
/// steht in [HShadcnTheme] und wird von dort in deren `ButtonTheme`-Einträge
/// gelegt.
HShadcnButtonRole _roleOf(HButtonVariant variant) => switch (variant) {
  HButtonVariant.primary => HShadcnButtonRole.primary,
  HButtonVariant.secondary => HShadcnButtonRole.secondary,
  HButtonVariant.ghost => HShadcnButtonRole.ghost,
  HButtonVariant.danger => HShadcnButtonRole.danger,
};

/// Dieselbe Vorschau, wie [HControl] sie kennt.
HControlPreview? _previewOf(HButtonPreview? preview) => switch (preview) {
  null => null,
  HButtonPreview.hovered => HControlPreview.hovered,
  HButtonPreview.pressed => HControlPreview.pressed,
  HButtonPreview.focused => HControlPreview.focused,
};

/// Button-Mindesthöhen. Beide erreichen das 28-px-Ziel des Designs.
///
/// Mindesthöhen und keine festen Höhen: bei `TextScaler.linear(2.0)` misst
/// `ui13` allein 40 px Zeilenhöhe, und eine feste Höhe schluckte den Überlauf
/// still (`docs/UX.md` 6 und 9, Punkt 18).
enum HButtonSize {
  /// 28 px, the density of a row or a toolbar.
  sm,

  /// 32 px, for a standalone action.
  md;

  /// Mindesthöhe in logischen Pixeln.
  double get minHeight =>
      this == HButtonSize.sm ? HSize.hitMin : HSize.hitDecision.height;

  /// Horizontal padding.
  double get padding => this == HButtonSize.sm ? HSpace.x2 + 2 : HSpace.x3;

  /// Vertical padding.
  double get verticalPadding => HSpace.x1;

  /// Die Mindesthöhe der Beschriftung, damit der Kasten [minHeight] erreicht.
  ///
  /// Der Kasten ist Beschriftung plus zweimal [verticalPadding]; der Rahmen
  /// zählt nicht dazu, weil die Bibliothek ihn in die Fläche malt statt ihn
  /// davorzulegen. Die Untergrenze steht innen und nicht außen, weil die
  /// Beschriftung sonst am oberen Rand klebte, sobald die Untergrenze greift.
  double get innerMinHeight => minHeight - 2 * verticalPadding;
}

/// An interaction state a button can be shown in without a pointer.
///
/// Exists for the gallery and for golden tests, which cannot hover or hold a
/// button down. Product code never sets it.
enum HButtonPreview {
  /// As if the pointer rested on the button.
  hovered,

  /// As if the button were held down.
  pressed,

  /// As if the button had keyboard focus.
  focused,
}

/// A button.
///
/// Steht über [HControl] auf `Clickable` aus `shadcn_flutter`, der Schicht,
/// aus der auch deren `Button` gemacht ist. Die Farben kommen aus [HTokens]
/// über [HShadcnButtonStyle], dieselbe Ableitung, die `HTheme` in die
/// `ButtonTheme`-Einträge der Bibliothek legt: was dieses Widget malt und was
/// ein Button der Bibliothek malt, kommt damit aus einer Quelle.
class HButton extends StatelessWidget {
  /// Creates a button whose label is [child].
  const HButton({
    required this.child,
    required this.onPressed,
    this.variant = HButtonVariant.secondary,
    this.size = HButtonSize.sm,
    this.leading,
    this.semanticsLabel,
    this.autofocus = false,
    this.focusNode,
    this.preview,
    super.key,
  });

  /// The label. Usually a `Text` the caller already localised.
  final Widget child;

  /// Invoked on tap, Enter and Space. A null callback disables the button.
  final VoidCallback? onPressed;

  /// Which role this button plays.
  final HButtonVariant variant;

  /// How tall the button is.
  final HButtonSize size;

  /// An optional glyph before the label.
  final Widget? leading;

  /// Screen-reader label, when the child is not descriptive enough.
  final String? semanticsLabel;

  /// Takes focus when first built.
  final bool autofocus;

  /// An externally owned focus node.
  final FocusNode? focusNode;

  /// Paints the button in this state regardless of the real pointer and focus.
  ///
  /// Null, the normal case, lets the widget track its own state. See
  /// [HButtonPreview].
  final HButtonPreview? preview;

  /// True when the button reacts to input.
  bool get enabled => onPressed != null;

  @override
  Widget build(BuildContext context) {
    final HShadcnButtonRole role = _roleOf(variant);
    final HTokens tokens = HTheme.of(context);
    return Semantics(
      button: true,
      enabled: enabled,
      label: semanticsLabel,
      child: HControl(
        onPressed: onPressed,
        focusNode: focusNode,
        autofocus: autofocus,
        preview: _previewOf(preview),
        leading: leading,
        leadingGap: HSpace.x2,
        radius: tokens.radii.control,
        fill: (HTokens tokens, Set<WidgetState> states) =>
            HShadcnButtonStyle.fillOf(tokens, role, states),
        style: (HTokens tokens, Color fill) => HShadcnButtonStyle.of(
          tokens,
          role,
          padding: EdgeInsets.symmetric(
            horizontal: size.padding,
            vertical: size.verticalPadding,
          ),
          fill: fill,
        ),
        builder: (BuildContext context, Set<WidgetState> states, Color fill) =>
            ConstrainedBox(
              constraints: BoxConstraints(minHeight: size.innerMinHeight),
              // widthFactor und heightFactor 1: der Button schrumpft auf seine
              // Beschriftung, auch wenn er in einer Spalte ohne feste Höhe
              // steht.
              child: Center(widthFactor: 1, heightFactor: 1, child: child),
            ),
      ),
    );
  }
}
