/// A label that appears beside its child while the pointer rests on it: the
/// tooltip of the icon rail. Lives in `core/ui` until `packages/ui` grows a
/// tooltip of its own (handoff of HUM-019).
library;

import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';

import 'ui.dart';

/// Where a [HoverLabel] appears relative to its child.
enum HoverLabelSide {
  /// To the right, vertically centred: the icon rail.
  right,

  /// Below, flush with the right edge: controls at the right end of the
  /// window, where a label to the right would leave the screen (HUM-255).
  below,
}

/// Shows [label] beside [child] after a short hover.
///
/// Needs an [Overlay] ancestor; the app provides one below its theme.
class HoverLabel extends StatefulWidget {
  /// Creates a hover label.
  const HoverLabel({
    required this.label,
    required this.child,
    this.delay = HMotion.hoverLabel,
    this.enabled = true,
    this.side = HoverLabelSide.right,
    super.key,
  });

  /// Where the label appears; to the right by default.
  final HoverLabelSide side;

  /// The text to show, already localised.
  final String label;

  /// The hovered widget.
  final Widget child;

  /// How long the pointer has to rest before the label appears.
  ///
  /// [HMotion.hoverLabel] by default; a duration never comes from a literal
  /// (`docs/UX.md` 2.1).
  final Duration delay;

  /// Whether the label appears at all.
  ///
  /// A switch rather than leaving the widget out: dropping it would change
  /// the parent of [child], and Flutter would rebuild the subtree below with
  /// fresh state. The expanded icon rail turns its labels off this way and
  /// keeps the fade of its names running (HUM-254).
  final bool enabled;

  @override
  State<HoverLabel> createState() => _HoverLabelState();
}

class _HoverLabelState extends State<HoverLabel> {
  final OverlayPortalController _portal = OverlayPortalController();
  final LayerLink _link = LayerLink();
  Timer? _timer;

  void _enter(PointerEnterEvent _) {
    _timer?.cancel();
    if (!widget.enabled) {
      return;
    }
    _timer = Timer(widget.delay, () {
      if (mounted) {
        _portal.show();
      }
    });
  }

  void _exit(PointerExitEvent _) {
    _timer?.cancel();
    _timer = null;
    if (_portal.isShowing) {
      _portal.hide();
    }
  }

  @override
  void didUpdateWidget(HoverLabel oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!widget.enabled) {
      _timer?.cancel();
      _timer = null;
      if (_portal.isShowing) {
        _portal.hide();
      }
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final bool below = widget.side == HoverLabelSide.below;
    return CompositedTransformTarget(
      link: _link,
      child: MouseRegion(
        onEnter: _enter,
        onExit: _exit,
        child: OverlayPortal(
          controller: _portal,
          overlayChildBuilder: (BuildContext context) =>
              CompositedTransformFollower(
                link: _link,
                targetAnchor: below
                    ? Alignment.bottomRight
                    : Alignment.centerRight,
                followerAnchor: below
                    ? Alignment.topRight
                    : Alignment.centerLeft,
                offset: below
                    ? Offset(0, tokens.spacing.x1)
                    : Offset(tokens.spacing.x2, 0),
                child: Align(
                  alignment: below ? Alignment.topRight : Alignment.centerLeft,
                  child: IgnorePointer(
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        color: tokens.colors.bg3,
                        borderRadius: BorderRadius.circular(
                          tokens.radii.control,
                        ),
                        border: Border.all(color: tokens.colors.lineStrong),
                      ),
                      child: Padding(
                        padding: EdgeInsets.symmetric(
                          horizontal: tokens.spacing.x2,
                          vertical: tokens.spacing.x1,
                        ),
                        child: Text(
                          widget.label,
                          style: tokens.typography.ui12.tinted(
                            tokens.colors.fg0,
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
          child: widget.child,
        ),
      ),
    );
  }
}
