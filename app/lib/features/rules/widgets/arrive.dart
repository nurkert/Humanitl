/// An arrival: eight pixels of travel plus a fade, over [HMotion.arrive] on
/// [HMotion.enter].
///
/// The one motion the banner, the undo strip and the sheet of this screen
/// use. From above for something that came in, from the edge it hangs on for
/// the sheet (`docs/UX.md` 2.2). Under reduced motion the travel is zero and
/// the fade keeps its full duration: less distance, not less feedback (2.10).
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';

/// Fades [child] in and slides it [HMotion.arriveOffset] into place.
class ArriveIn extends StatefulWidget {
  /// Wraps [child].
  const ArriveIn({required this.child, this.fromRight = false, super.key});

  /// What arrives.
  final Widget child;

  /// True for something that hangs on the right edge, false for something
  /// that came from above.
  final bool fromRight;

  @override
  State<ArriveIn> createState() => _ArriveInState();
}

class _ArriveInState extends State<ArriveIn>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: HMotion.arrive,
  );
  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _controller,
    curve: HMotion.enter,
  );
  bool _done = false;

  @override
  void initState() {
    super.initState();
    _controller
      ..addStatusListener(_finish)
      ..forward();
  }

  void _finish(AnimationStatus status) {
    // The wrapper leaves the tree as soon as it has nothing left to do
    // (`docs/UX.md` 7).
    if (status == AnimationStatus.completed && mounted && !_done) {
      setState(() => _done = true);
    }
  }

  @override
  void dispose() {
    _curve.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_done) {
      return widget.child;
    }
    final double distance = HReducedMotion.distance(
      context,
      HMotion.arriveOffset,
    );
    final Widget faded = FadeTransition(opacity: _curve, child: widget.child);
    if (distance == 0) {
      return faded;
    }
    // `SlideTransition` measures its offset as a fraction of the child, and
    // this distance is eight logical pixels of a box whose height nobody
    // knows in advance. The child is handed to the builder rather than built
    // inside it, so only the transform is rebuilt per frame -- which is the
    // reason `docs/UX.md` 7 prefers the transitions in the first place.
    return AnimatedBuilder(
      animation: _curve,
      child: faded,
      builder: (BuildContext context, Widget? child) => Transform.translate(
        offset: widget.fromRight
            ? Offset(distance * (1 - _curve.value), 0)
            : Offset(0, -distance * (1 - _curve.value)),
        child: child,
      ),
    );
  }
}
