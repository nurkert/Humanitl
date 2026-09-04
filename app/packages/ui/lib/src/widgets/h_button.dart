import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/colors.dart';
import '../tokens/flow_state.dart';
import '../tokens/motion.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';
import 'h_animated_fill.dart';
import 'h_focus_ring.dart';

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
const double _dangerHoverAlpha = HColors.fillHoverAlpha;

/// Area alpha of the blocked hue behind a pressed danger button.
const double _dangerPressedAlpha = HColors.fillPressedAlpha;

/// A button.
///
/// Hover, press and focus are all rendered; the press fill takes
/// [HMotion.press], which is the only feedback the design allows itself.
/// Der Fokus kommt als [HFocusRing]: zwei Pixel Akzent außerhalb des eigenen
/// Rahmens, in einem Frame, nie als umgefärbter Rahmen (`docs/UX.md` 6).
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
        // Nicht [HSurfaceColors.accent], sondern die Füllung: Weiß auf dem
        // hellen Akzent misst 3,73:1, und der Ruhezustand ist der Normalfall
        // des einen gefüllten Controls je Bildschirm. Die Füllung weicht
        // zurück, bis [HSurfaceColors.onAccent] 4,5:1 erreicht; im dunklen
        // Theme ist sie der Akzent selbst (`docs/UX.md` 6).
        final Color fill = c.accentFill;
        return _HButtonPalette(
          background: fill,
          hover: HColorDerivation.darken(fill, hoverStep),
          pressed: HColorDerivation.darken(fill, 0.06),
          foreground: c.onAccent,
          border: fill,
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
        // Fläche und Beschriftung werden getrennt geführt: die Füllung ist die
        // Zustandsfarbe, das Wort die Textvariante, die auf jeder der drei
        // Füllungen 4,5:1 erreicht (`docs/UX.md` 6).
        final Color blocked = tokens.state.blocked;
        return _HButtonPalette(
          background: HColorDerivation.tint(blocked, _dangerRestAlpha),
          hover: blocked.withValues(alpha: _dangerHoverAlpha),
          pressed: blocked.withValues(alpha: _dangerPressedAlpha),
          foreground: tokens.stateTextColor(HFlowState.blocked),
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
    // Eine Mindesthöhe, keine Höhe: bei doppelter Textskalierung wächst der
    // Button mit seiner Beschriftung, statt sie abzuschneiden
    // (`docs/UX.md` 6).
    // Kein `AnimatedContainer`: der baut seinen Controller ohne
    // `animationBehavior` und verlöre die 120 ms der Tastenfüllung, sobald
    // die Plattform `disableAnimations` meldet (`docs/UX.md` 2.10).
    Widget button = HAnimatedFill(
      color: background,
      builder: (BuildContext context, Color fill) => Container(
        constraints: BoxConstraints(minHeight: widget.size.minHeight),
        padding: EdgeInsets.symmetric(
          horizontal: widget.size.padding,
          vertical: tokens.spacing.x1,
        ),
        decoration: BoxDecoration(
          color: fill,
          borderRadius: HRadius.controlRadius,
          // Der Rahmen bleibt der Rahmen. Fokus zeigt der Ring außerhalb.
          border: Border.all(color: palette.border),
        ),
        // heightFactor: 1: der Button schrumpft auf seine Beschriftung, auch
        // wenn er in einer Spalte ohne feste Höhe steht.
        child: Center(widthFactor: 1, heightFactor: 1, child: content),
      ),
    );
    // Der Ring liegt außerhalb des Rahmens und erscheint in einem Frame; sein
    // Platz ist immer reserviert, also verschiebt der Fokus nichts.
    button = HFocusRing(
      visible: focused && enabled,
      radius: tokens.radii.control,
      // Der Primärbutton ist mit dem Akzent gefüllt, und der Ring ist der
      // Akzent: ohne die zwei Pixel Fläche dazwischen stünde er bei 1,00:1
      // gegen seine eigene Füllung (`docs/UX.md` 6).
      over: background,
      child: button,
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
