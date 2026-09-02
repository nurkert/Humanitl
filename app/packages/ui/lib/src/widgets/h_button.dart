import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/colors.dart';
import '../tokens/motion.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';

/// The four button roles. There is no fifth.
enum HButtonVariant {
  /// The one action a surface wants: filled with the accent.
  primary,

  /// A second, equal action: a tinted surface with a hairline.
  secondary,

  /// A quiet action: no surface until hovered.
  ghost,

  /// A destructive action, in the blocked hue. Never for "Block" itself —
  /// blocking is a normal decision, not a destructive one.
  danger,
}

/// Button heights. Both clear the 28 px hit-target minimum.
enum HButtonSize {
  /// 28 px, the density of a row or a toolbar.
  sm,

  /// 32 px, for a standalone action.
  md;

  /// Height in logical pixels.
  double get height => this == HButtonSize.sm ? HSize.hitMin : 32;

  /// Horizontal padding.
  double get padding => this == HButtonSize.sm ? HSpace.x2 + 2 : HSpace.x3;
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

/// Area alpha of the blocked hue behind a danger button at rest: the tint cap
/// of the design, [HColors.tintAlpha].
const double _dangerRestAlpha = HColors.tintAlpha;

/// Area alpha of the blocked hue behind a hovered danger button.
///
/// Hover and press step above the resting tint the way the secondary variant
/// steps from `bg2` to `bg3`; without the step the three states are one and
/// the same fill. The steps are as small as they can be while still visible:
/// the label, drawn in the blocked hue, has to keep 3:1 over the fill on every
/// surface of both ladders, and the light pressed fill is the tight case.
const double _dangerHoverAlpha = 0.14;

/// Area alpha of the blocked hue behind a pressed danger button.
const double _dangerPressedAlpha = 0.18;

/// A button.
///
/// Hover, press and focus are all rendered; the press fill takes
/// [HMotion.press], which is the only feedback the design allows itself.
class HButton extends StatefulWidget {
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
  State<HButton> createState() => _HButtonState();
}

class _HButtonState extends State<HButton> {
  bool _hovered = false;
  bool _pressed = false;
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

  void _setPressed(bool value) {
    if (_pressed != value) {
      setState(() => _pressed = value);
    }
  }

  _HButtonPalette _palette(HTokens tokens) {
    final HSurfaceColors c = tokens.colors;
    switch (widget.variant) {
      case HButtonVariant.primary:
        // Hover steps the accent away from the surface it sits on: lighter in
        // the dark theme, darker in the light one. Lightening the light accent
        // would drop it below 3:1 on the highest light surface.
        final double hoverStep = tokens.brightness == Brightness.dark
            ? -0.04
            : 0.04;
        return _HButtonPalette(
          background: c.accent,
          hover: HColorDerivation.darken(c.accent, hoverStep),
          pressed: HColorDerivation.darken(c.accent, 0.06),
          foreground: c.onAccent,
          border: c.accent,
        );
      case HButtonVariant.secondary:
        return _HButtonPalette(
          background: c.bg2,
          hover: c.bg3,
          pressed: c.bg3,
          foreground: c.fg0,
          border: c.line,
        );
      case HButtonVariant.ghost:
        return _HButtonPalette(
          background: const Color(0x00000000),
          hover: c.bg2,
          pressed: c.bg3,
          foreground: c.fg1,
          border: const Color(0x00000000),
        );
      case HButtonVariant.danger:
        // Three distinct fills of the same hue; see the alpha constants above.
        final Color blocked = tokens.state.blocked;
        return _HButtonPalette(
          background: HColorDerivation.tint(blocked, _dangerRestAlpha),
          hover: blocked.withValues(alpha: _dangerHoverAlpha),
          pressed: blocked.withValues(alpha: _dangerPressedAlpha),
          foreground: blocked,
          border: HColorDerivation.fade(blocked, 0.4),
        );
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final _HButtonPalette palette = _palette(tokens);
    final bool enabled = widget.enabled;
    final HButtonPreview? preview = widget.preview;
    final bool hovered = preview == null
        ? _hovered
        : preview == HButtonPreview.hovered;
    final bool pressed = preview == null
        ? _pressed
        : preview == HButtonPreview.pressed;
    final bool focused = preview == null
        ? _focused
        : preview == HButtonPreview.focused;
    final Color background = !enabled
        ? palette.background
        : pressed
        ? palette.pressed
        : hovered
        ? palette.hover
        : palette.background;

    Widget content = DefaultTextStyle(
      style: tokens.typography.ui13.medium.tinted(palette.foreground),
      child: widget.child,
    );
    final Widget? leading = widget.leading;
    if (leading != null) {
      content = Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          leading,
          SizedBox(width: tokens.spacing.x2),
          content,
        ],
      );
    }

    // The button shrink-wraps its label; giving the container an alignment
    // would make it fill whatever column it is dropped into.
    Widget button = AnimatedContainer(
      duration: HMotion.press,
      curve: HMotion.enter,
      height: widget.size.height,
      padding: EdgeInsets.symmetric(horizontal: widget.size.padding),
      decoration: BoxDecoration(
        color: background,
        borderRadius: HRadius.controlRadius,
        border: Border.all(
          color: focused ? tokens.colors.accent : palette.border,
        ),
      ),
      child: Center(widthFactor: 1, child: content),
    );
    if (!enabled) {
      button = Opacity(opacity: 0.45, child: button);
    }

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
        onShowFocusHighlight: _setFocused,
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (ActivateIntent intent) {
              widget.onPressed?.call();
              return null;
            },
          ),
        },
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: enabled ? widget.onPressed : null,
          onTapDown: enabled ? (TapDownDetails _) => _setPressed(true) : null,
          onTapUp: enabled ? (TapUpDetails _) => _setPressed(false) : null,
          onTapCancel: enabled ? () => _setPressed(false) : null,
          child: button,
        ),
      ),
    );
  }
}

@immutable
class _HButtonPalette {
  const _HButtonPalette({
    required this.background,
    required this.hover,
    required this.pressed,
    required this.foreground,
    required this.border,
  });

  final Color background;
  final Color hover;
  final Color pressed;
  final Color foreground;
  final Color border;
}
