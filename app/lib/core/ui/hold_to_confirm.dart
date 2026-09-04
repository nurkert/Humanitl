/// Press and hold: a fill that grows from the left and confirms when it is
/// full.
///
/// Two rows of the motion table use it (`docs/UX.md` 2.2): 250 ms
/// [HMotion.holdToBlock] confirms a block, 400 ms [HMotion.holdToConfirm]
/// fills the release valve. A fill that grows from the left is progress, not
/// direction: it says "this far", not "that way", so it does not break the
/// axis rule of 2.3.
///
/// The hold runs on its own timer over `onPointerDown`/`onPointerUp`, not on
/// `GestureDetector.onLongPress`: the long press recogniser brings its own
/// 500 ms and would decide the duration for us (pitfall of HUM-028).
///
/// A hold belongs to what it was started on. [HoldToConfirm.token] carries
/// that identity -- the flow, or the whole selection -- and a hold whose token
/// changes while the finger is down is cancelled instead of completed:
/// otherwise somebody holds the control for one request and lets go onto
/// another (`docs/UX.md` 5.4).
library;

import 'package:flutter/widgets.dart';

import 'ui.dart';

/// A control whose action needs the pointer to stay down for [duration].
class HoldToConfirm extends StatefulWidget {
  /// Creates a hold target.
  const HoldToConfirm({
    required this.duration,
    required this.fill,
    required this.onConfirmed,
    required this.builder,
    this.onTapShort,
    this.enabled = true,
    this.previewProgress,
    this.token,
    super.key,
  });

  /// How long the pointer has to stay down.
  final Duration duration;

  /// Colour of the growing fill.
  final Color fill;

  /// Called once, when the hold completed.
  final VoidCallback onConfirmed;

  /// Called when the pointer came up before [duration] had passed. A short
  /// press is never silent: either it does the smaller thing or it says why
  /// it did nothing (`docs/UX.md` 5.3).
  final VoidCallback? onTapShort;

  /// False disables both the hold and the short press.
  final bool enabled;

  /// Draws the control; [progress] runs from zero to one while held.
  final Widget Function(BuildContext context, double progress) builder;

  /// Paints a fixed progress instead of tracking the pointer. For goldens,
  /// which cannot hold a control down. Product code leaves it null.
  final double? previewProgress;

  /// What this hold belongs to: the flow, or the selection it would decide.
  ///
  /// A running hold is cancelled as soon as the token changes, so a
  /// confirmation can never land on something else than what it was started
  /// on. Null means the caller has nothing to tie the hold to.
  final Object? token;

  @override
  State<HoldToConfirm> createState() => _HoldToConfirmState();
}

class _HoldToConfirmState extends State<HoldToConfirm>
    with SingleTickerProviderStateMixin {
  // `AnimationBehavior.preserve`, and it is the whole protection: with the
  // normal behaviour Flutter scales every duration to five percent as soon as
  // the platform reports `disableAnimations`, so 400 ms become 20 ms and an
  // ordinary click confirms. The hold is not an animation that decorates, it
  // is the time in which a decision is taken back (`docs/UX.md` 2.10, 4.7,
  // 5.4).
  late final AnimationController _hold = AnimationController(
    vsync: this,
    duration: widget.duration,
    animationBehavior: AnimationBehavior.preserve,
  );
  bool _fired = false;

  /// The token the running hold was started on.
  Object? _startedOn;

  @override
  void initState() {
    super.initState();
    _hold.addStatusListener(_onStatus);
  }

  @override
  void didUpdateWidget(HoldToConfirm old) {
    super.didUpdateWidget(old);
    if (widget.token != old.token && _hold.isAnimating) {
      // What the hold was started on is gone; the fill runs back and nothing
      // is decided.
      _fired = true;
      _hold.reverse();
    }
  }

  @override
  void dispose() {
    _hold
      ..removeStatusListener(_onStatus)
      ..dispose();
    super.dispose();
  }

  void _onStatus(AnimationStatus status) {
    if (status != AnimationStatus.completed || _fired) {
      return;
    }
    _fired = true;
    if (_startedOn != widget.token) {
      // The selection moved under the finger between the last frame and this
      // one; a hold decides what it was started on, or nothing.
      _hold.reverse();
      return;
    }
    widget.onConfirmed();
  }

  void _down(PointerDownEvent event) {
    if (!widget.enabled) {
      return;
    }
    _fired = false;
    _startedOn = widget.token;
    _hold.forward(from: 0);
  }

  void _up(PointerUpEvent event) {
    if (!widget.enabled) {
      return;
    }
    final bool short = !_fired;
    // The fill runs back the way it came; the press duration is the only
    // feedback the design allows itself.
    _hold.reverse();
    if (short) {
      widget.onTapShort?.call();
    }
  }

  void _cancel(PointerCancelEvent event) => _hold.reverse();

  @override
  Widget build(BuildContext context) {
    final double? preview = widget.previewProgress;
    return Listener(
      behavior: HitTestBehavior.opaque,
      onPointerDown: _down,
      onPointerUp: _up,
      onPointerCancel: _cancel,
      child: preview != null
          ? _fillOf(context, preview)
          : AnimatedBuilder(
              animation: _hold,
              builder: (BuildContext context, Widget? _) =>
                  _fillOf(context, _hold.value),
            ),
    );
  }

  Widget _fillOf(BuildContext context, double progress) {
    final Widget child = widget.builder(context, progress);
    if (progress <= 0) {
      return child;
    }
    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          colors: <Color>[
            widget.fill,
            widget.fill,
            const Color(0x00000000),
            const Color(0x00000000),
          ],
          stops: <double>[0, progress, progress, 1],
        ),
      ),
      child: child,
    );
  }
}

/// The alpha of a hold fill: [HColors.fillHoldAlpha].
///
/// Not a fourth number of its own. The contrast test has to enumerate every
/// surface a state colour can sit behind its own text, so every fill alpha of
/// the system lives in `HColors` and appears in
/// [HColorDerivation.fillAlphas]; the split pill fills its hold with the same
/// token (`docs/UX.md` 6).
const double holdFillAlpha = HColors.fillHoldAlpha;

/// [color] as the fill of a hold, over a surface that already carries
/// [beneath] of the same colour.
///
/// Zwei Schichten derselben Farbe addieren sich nicht, sie komponieren: 0,20
/// über einer Tönung von 0,06 sind wirksam 0,248, und für diese Fläche gilt
/// keine der Zusicherungen der Textableitung — die Beschriftung fällt dort
/// auf 4,14:1. Wer auf einer schon getönten Fläche hält, nennt deshalb ihr
/// Alpha, und die Füllung rechnet sich zurück
/// ([HColorDerivation.alphaOver], `docs/UX.md` 6). Null ist der Normalfall:
/// die Aktionsleiste hält auf `bg1`.
Color holdFill(Color color, {double beneath = 0}) =>
    color.withValues(alpha: HColorDerivation.alphaOver(holdFillAlpha, beneath));
