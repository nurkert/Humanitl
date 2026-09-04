/// Block: the ghost half of the decision.
///
/// Blocking is drawn quieter than allowing although it looks like the harder
/// action, because the agent may retry it; allowing is the one that cannot be
/// taken back (`docs/UX.md` 5.4). The pointer has to stay down for
/// [HMotion.holdToBlock], so that a hurried click cannot take the decision;
/// the key `B` takes it at once, because a key press cannot slip sideways into
/// the wrong control.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';

import '../../../core/ui/focus_ring.dart';
import '../../../core/ui/hold_to_confirm.dart';
import '../../../core/ui/ui.dart';

/// The block control.
class BlockButton extends StatefulWidget {
  /// Creates the control.
  const BlockButton({
    required this.label,
    required this.shortcutHint,
    required this.semanticsValue,
    required this.onBlock,
    this.onShortPress,
    this.onHighlight,
    this.enabled = true,
    this.pressed = false,
    this.refusals = 0,
    this.previewHold,
    this.holdToken,
    super.key,
  });

  /// What the control does, in the person's language.
  final String label;

  /// The key that does the same thing, shown on the control itself.
  final String shortcutHint;

  /// The remaining hold budget, for the screen reader.
  final String semanticsValue;

  /// Blocks the request.
  final VoidCallback onBlock;

  /// The pointer came up before the hold was through. The bar says why the
  /// click did nothing; the control itself stays quiet (`docs/UX.md` 5.3).
  final VoidCallback? onShortPress;

  /// True while the pointer rests on the control or it has the focus. The
  /// sentence above the bar follows it: whoever reaches for Block reads the
  /// rule Block would create, not the one the valve would.
  final ValueChanged<bool>? onHighlight;

  /// False greys the control out.
  final bool enabled;

  /// True while this control's decision is on its way to the daemon: it shows
  /// the same fill a press shows, and keeps it until the answer arrives.
  final bool pressed;

  /// How many inputs were refused so far; a change fills the control once.
  final int refusals;

  /// Paints the hold at a fixed progress, for goldens.
  final double? previewHold;

  /// What a running hold belongs to: the flow, or the selection it decides.
  /// A hold whose token changes is cancelled, never completed
  /// (`docs/UX.md` 5.4).
  final Object? holdToken;

  @override
  State<BlockButton> createState() => _BlockButtonState();
}

class _BlockButtonState extends State<BlockButton> {
  bool _focused = false;
  bool _hovered = false;
  bool _refusing = false;
  Timer? _refusal;

  @override
  void didUpdateWidget(BlockButton old) {
    super.didUpdateWidget(old);
    if (widget.refusals != old.refusals) {
      _refusal?.cancel();
      setState(() => _refusing = true);
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
    final Color blocked = tokens.state.blocked;
    return Opacity(
      opacity: widget.enabled ? 1 : 0.45,
      child: FocusRing(
        visible: _focused,
        radius: tokens.radii.control,
        child: FocusableActionDetector(
          enabled: widget.enabled,
          mouseCursor: widget.enabled
              ? SystemMouseCursors.click
              : MouseCursor.defer,
          onFocusChange: (bool value) {
            setState(() => _focused = value);
            widget.onHighlight?.call(value || _hovered);
          },
          onShowHoverHighlight: (bool value) {
            setState(() => _hovered = value);
            widget.onHighlight?.call(value || _focused);
          },
          actions: <Type, Action<Intent>>{
            ActivateIntent: CallbackAction<ActivateIntent>(
              onInvoke: (ActivateIntent intent) {
                // A key is the second way to the same decision, and it does
                // not need the hold: the hold protects against a click that
                // went to the wrong control (`docs/UX.md` 5.1, 5.4).
                widget.onBlock();
                return null;
              },
            ),
          },
          // One node for the whole control: label the action, value the
          // deadline. The countdown never enters the label, or a screen
          // reader repeats the whole line every second (`docs/UX.md` 6).
          child: Semantics(
            container: true,
            button: true,
            enabled: widget.enabled,
            label: widget.label,
            value: widget.semanticsValue,
            child: HoldToConfirm(
              key: const Key('intercept-block-hold'),
              duration: HMotion.holdToBlock,
              fill: holdFill(blocked),
              enabled: widget.enabled,
              token: widget.holdToken,
              previewProgress: widget.previewHold,
              onConfirmed: widget.onBlock,
              onTapShort: widget.onShortPress,
              builder: (BuildContext context, double progress) {
                // While the fill runs under the label, the label steps to the
                // neutral text colour: the blocked hue on its own fill reaches
                // 3,4:1 in the light theme, and every sentence somebody reads
                // is 4,5:1 (`docs/UX.md` 6).
                final Color foreground = progress > 0
                    ? tokens.colors.fg0
                    : blocked;
                return AnimatedContainer(
                  duration: HMotion.press,
                  curve: HMotion.enter,
                  constraints: BoxConstraints(
                    minWidth: tokens.sizes.hitDecision.width,
                    minHeight: tokens.sizes.hitDecision.height,
                  ),
                  padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
                  decoration: BoxDecoration(
                    // Ghost: a border and a label, never an area
                    // (`docs/UX.md` 3.1). Hover and the refused input are the
                    // two moments it fills, and both run empty again.
                    color: _refusing || widget.pressed
                        ? blocked.withValues(alpha: HColors.tintAlpha)
                        : _hovered
                        ? blocked.withValues(alpha: 0.06)
                        : const Color(0x00000000),
                    borderRadius: BorderRadius.circular(tokens.radii.control),
                    border: Border.all(
                      color: HColorDerivation.fade(blocked, 0.4),
                    ),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      HGlyphIcon(
                        HGlyph.shieldX,
                        size: 14,
                        color: progress > 0 ? tokens.colors.fg0 : blocked,
                      ),
                      SizedBox(width: tokens.spacing.x2),
                      Flexible(
                        child: ExcludeSemantics(
                          child: Text(
                            widget.label,
                            key: const Key('intercept-block-label'),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: tokens.typography.ui13.medium.tinted(
                              foreground,
                            ),
                          ),
                        ),
                      ),
                      SizedBox(width: tokens.spacing.x2),
                      ExcludeSemantics(
                        child: Text(
                          widget.shortcutHint,
                          style: tokens.typography.mono11.tinted(
                            tokens.colors.fg1,
                          ),
                        ),
                      ),
                    ],
                  ),
                );
              },
            ),
          ),
        ),
      ),
    );
  }
}
