/// A label that appears beside its child while the pointer rests on it: the
/// tooltip of the icon rail. Lives in `core/ui` until `packages/ui` grows a
/// tooltip of its own (handoff of HUM-019).
library;

import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';

import 'ui.dart';

/// Shows [label] to the right of [child] after a short hover.
///
/// Needs an [Overlay] ancestor; the app provides one below its theme.
class HoverLabel extends StatefulWidget {
  /// Creates a hover label.
  const HoverLabel({
    required this.label,
    required this.child,
    this.delay = const Duration(milliseconds: 350),
    super.key,
  });

  /// The text to show, already localised.
  final String label;

  /// The hovered widget.
  final Widget child;

  /// How long the pointer has to rest before the label appears.
  final Duration delay;

  @override
  State<HoverLabel> createState() => _HoverLabelState();
}

class _HoverLabelState extends State<HoverLabel> {
  final OverlayPortalController _portal = OverlayPortalController();
  final LayerLink _link = LayerLink();
  Timer? _timer;

  void _enter(PointerEnterEvent _) {
    _timer?.cancel();
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
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
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
                targetAnchor: Alignment.centerRight,
                followerAnchor: Alignment.centerLeft,
                offset: Offset(tokens.spacing.x2, 0),
                child: Align(
                  alignment: Alignment.centerLeft,
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
