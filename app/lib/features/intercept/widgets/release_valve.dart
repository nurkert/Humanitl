/// The release valve: the one filled control of the screen (`docs/UX.md` 3.1).
///
/// A split pill. The left half sends the request; holding it for
/// [HMotion.holdToConfirm] sends it and remembers the decision. The right half
/// opens the grid that says how long and how far the rule reaches. Allowing is
/// irreversible, so its protection sits in time, not in a modal and not in a
/// smaller hit target (`docs/UX.md` 5.4).
library;

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/widgets.dart';

import '../../../core/ui/focus_ring.dart';
import '../../../core/ui/hold_to_confirm.dart';
import '../../../core/ui/ui.dart';

/// Width of the chevron segment.
const double valveChevronWidth = HSize.hitMin;

/// Area alpha the left half adds while the pointer rests on it.
///
/// The pill already carries the accent at the tint cap; hover steps above it
/// the way the secondary button steps from `bg2` to `bg3`.
const double valveHoverAlpha = 0.06;

/// Area alpha the left half shows for one press, and for one refusal.
const double valvePressAlpha = 0.12;

/// The release valve.
class ReleaseValve extends StatefulWidget {
  /// Creates the valve.
  const ReleaseValve({
    required this.label,
    required this.holdLabel,
    required this.shortcutHint,
    required this.semanticsValue,
    required this.optionsLabel,
    required this.onAllow,
    required this.onAllowRemembered,
    required this.onToggleOptions,
    this.onShortPress,
    this.holdRequired = false,
    this.accent,
    this.enabled = true,
    this.pressed = false,
    this.optionsOpen = false,
    this.refusals = 0,
    this.previewHold,
    this.holdToken,
    super.key,
  });

  /// What pressing the left half does, in the person's language.
  final String label;

  /// What holding the left half does; shown while it is held.
  final String holdLabel;

  /// The key that does the same thing, shown on the control itself.
  final String shortcutHint;

  /// The remaining hold budget, for the screen reader.
  final String semanticsValue;

  /// Screen-reader label of the chevron half.
  final String optionsLabel;

  /// Sends the request once, unchanged.
  final VoidCallback onAllow;

  /// Sends the request and creates the rule the label names.
  final VoidCallback onAllowRemembered;

  /// Shows or hides the grid.
  final VoidCallback onToggleOptions;

  /// The pointer came up before the hold was through, while the hold was
  /// required. The bar says why the click did nothing (`docs/UX.md` 5.3).
  final VoidCallback? onShortPress;

  /// True while sending needs the same hold as blocking: a request with an
  /// unresolved finding (`docs/UX.md` 4.7). A click then does nothing but say
  /// why, and the hold sends.
  final bool holdRequired;

  /// The hue of the control. The accent of the theme when null; amber while a
  /// finding is unresolved, because that request must not look like a routine
  /// one (`docs/UX.md` 4.7).
  final Color? accent;

  /// False greys the control out; the request is no longer decidable.
  final bool enabled;

  /// True while this control's decision is on its way to the daemon: it shows
  /// the same fill a press shows, and keeps it until the answer arrives.
  final bool pressed;

  /// Whether the grid is currently shown.
  final bool optionsOpen;

  /// How many inputs were refused so far. A change fills the control and lets
  /// it run empty again: a refused input is never silent (`docs/UX.md` 5.3).
  final int refusals;

  /// Paints the hold at a fixed progress, for goldens.
  final double? previewHold;

  /// What a running hold belongs to: the flow, or the selection it decides.
  /// A hold whose token changes is cancelled, never completed
  /// (`docs/UX.md` 5.4).
  final Object? holdToken;

  @override
  State<ReleaseValve> createState() => _ReleaseValveState();
}

class _ReleaseValveState extends State<ReleaseValve> {
  bool _leftFocused = false;
  bool _rightFocused = false;
  bool _hovered = false;
  bool _refusing = false;
  Timer? _refusal;

  @override
  void didUpdateWidget(ReleaseValve old) {
    super.didUpdateWidget(old);
    if (widget.refusals != old.refusals) {
      _refusal?.cancel();
      setState(() => _refusing = true);
      // Fill for one press duration, then empty again. The container carries
      // both halves of the motion; nothing here decides how fast.
      _refusal = Timer(HMotion.press, () {
        if (mounted) {
          setState(() => _refusing = false);
        }
      });
    }
  }

  @override
  void dispose() {
    _refusal?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color accent = widget.accent ?? tokens.colors.accent;
    final double radius = tokens.sizes.hitDecision.height / 2;
    return Opacity(
      opacity: widget.enabled ? 1 : 0.45,
      child: FocusRing(
        visible: _leftFocused || _rightFocused,
        radius: radius,
        child: ConstrainedBox(
          constraints: BoxConstraints(
            minWidth: tokens.sizes.hitDecision.width,
            minHeight: tokens.sizes.hitDecision.height,
          ),
          child: DecoratedBox(
            decoration: BoxDecoration(
              // The one filled control of the screen: the accent as an area,
              // at the tint cap of the token layer (`docs/UX.md` 3.1 and 8).
              color: tokens.tint(accent),
              borderRadius: BorderRadius.circular(radius),
              border: Border.all(color: HColorDerivation.fade(accent, 0.4)),
            ),
            child: ClipRRect(
              borderRadius: BorderRadius.circular(radius),
              // Both halves are as tall as the taller one, whatever the text
              // scale makes of it: [HSize.hitDecision] is a minimum, not a
              // height (`docs/UX.md` 6).
              child: IntrinsicHeight(
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    Flexible(child: _left(tokens)),
                    HHairline(
                      vertical: true,
                      color: HColorDerivation.fade(accent, 0.4),
                    ),
                    _right(tokens),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _left(HTokens tokens) {
    final Color accent = widget.accent ?? tokens.colors.accent;
    final Color surface = _refusing || widget.pressed
        ? accent.withValues(alpha: valvePressAlpha)
        : _hovered
        ? accent.withValues(alpha: valveHoverAlpha)
        : const Color(0x00000000);
    return FocusableActionDetector(
      enabled: widget.enabled,
      mouseCursor: widget.enabled
          ? SystemMouseCursors.click
          : MouseCursor.defer,
      onFocusChange: (bool value) => setState(() => _leftFocused = value),
      onShowHoverHighlight: (bool value) => setState(() => _hovered = value),
      actions: <Type, Action<Intent>>{
        ActivateIntent: CallbackAction<ActivateIntent>(
          onInvoke: (ActivateIntent intent) {
            widget.onAllow();
            return null;
          },
        ),
      },
      // One node for the whole half: the label says what pressing it does,
      // the value carries the deadline. The countdown never enters the label,
      // or a screen reader repeats the line every second (`docs/UX.md` 6).
      child: Semantics(
        container: true,
        button: true,
        enabled: widget.enabled,
        label: widget.label,
        value: widget.semanticsValue,
        child: HoldToConfirm(
          key: const Key('intercept-valve-hold'),
          duration: HMotion.holdToConfirm,
          fill: holdFill(tokens.state.allowed),
          enabled: widget.enabled,
          token: widget.holdToken,
          previewProgress: widget.previewHold,
          // While a finding is unresolved the hold *is* the send, and a click
          // only says why it did nothing (`docs/UX.md` 4.7). Otherwise the
          // click sends and the hold sends and remembers.
          onConfirmed: widget.holdRequired
              ? widget.onAllow
              : widget.onAllowRemembered,
          onTapShort: widget.holdRequired
              ? widget.onShortPress
              : widget.onAllow,
          builder: (BuildContext context, double progress) => AnimatedContainer(
            duration: HMotion.press,
            curve: HMotion.enter,
            color: surface,
            constraints: BoxConstraints(
              minHeight: tokens.sizes.hitDecision.height,
            ),
            padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Flexible(
                  child: ExcludeSemantics(
                    child: Text(
                      progress > 0 ? widget.holdLabel : widget.label,
                      key: const Key('intercept-valve-label'),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: tokens.typography.ui13.medium.tinted(
                        tokens.colors.fg0,
                      ),
                    ),
                  ),
                ),
                SizedBox(width: tokens.spacing.x2),
                ExcludeSemantics(
                  child: Text(
                    widget.shortcutHint,
                    style: tokens.typography.mono11.tinted(tokens.colors.fg1),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _right(HTokens tokens) => FocusableActionDetector(
    enabled: widget.enabled,
    mouseCursor: widget.enabled ? SystemMouseCursors.click : MouseCursor.defer,
    onFocusChange: (bool value) => setState(() => _rightFocused = value),
    actions: <Type, Action<Intent>>{
      ActivateIntent: CallbackAction<ActivateIntent>(
        onInvoke: (ActivateIntent intent) {
          widget.onToggleOptions();
          return null;
        },
      ),
    },
    child: Semantics(
      button: true,
      enabled: widget.enabled,
      expanded: widget.optionsOpen,
      label: widget.optionsLabel,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: widget.enabled ? widget.onToggleOptions : null,
        child: ConstrainedBox(
          key: const Key('intercept-valve-options'),
          constraints: BoxConstraints(
            minWidth: valveChevronWidth,
            maxWidth: valveChevronWidth,
            minHeight: tokens.sizes.hitDecision.height,
          ),
          child: Center(
            // The chevron turns with the grid; it does not animate. A quarter
            // turn is a state, not an event (`docs/UX.md` 2.2).
            child: Transform.rotate(
              angle: widget.optionsOpen ? math.pi / 2 : 0,
              child: HGlyphIcon(
                HGlyph.chevronRight,
                size: 14,
                color: widget.accent ?? tokens.colors.accent,
              ),
            ),
          ),
        ),
      ),
    ),
  );
}
