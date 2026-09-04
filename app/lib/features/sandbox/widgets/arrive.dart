/// An arrival: a fade over [HMotion.arrive] on [HMotion.enter], with the
/// travel the thing that arrived is entitled to.
///
/// Three things on this screen appear: the finding under the header comes
/// from above like anything that arrived, the command sheet comes out of the
/// edge it hangs on, and the modal comes with no travel at all -- a modal
/// that flies in claims a direction it does not have (`docs/UX.md` 2.2).
/// Under reduced motion the travel is zero and the fade keeps its full
/// duration: less distance, not less feedback (2.10).
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';

/// Where something came from.
enum ArriveFrom {
  /// From above, like a row arriving in the queue.
  top,

  /// Out of the right edge, where a sheet hangs.
  right,

  /// From nowhere: a fade in place.
  nowhere,
}

/// Fades [child] in and slides it into place.
class SandboxArrive extends StatefulWidget {
  /// Wraps [child].
  const SandboxArrive({
    required this.child,
    this.from = ArriveFrom.top,
    super.key,
  });

  /// What arrives.
  final Widget child;

  /// Which edge it came from.
  final ArriveFrom from;

  @override
  State<SandboxArrive> createState() => _SandboxArriveState();
}

class _SandboxArriveState extends State<SandboxArrive>
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

  /// The wrapper leaves the tree as soon as it has nothing left to do
  /// (`docs/UX.md` 7).
  void _finish(AnimationStatus status) {
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
    final double travel = widget.from == ArriveFrom.nowhere
        ? 0
        : HReducedMotion.distance(context, HMotion.arriveOffset);
    return FadeTransition(
      opacity: _curve,
      child: AnimatedBuilder(
        animation: _curve,
        builder: (BuildContext context, Widget? child) {
          final double left = 1 - _curve.value;
          final Offset offset = switch (widget.from) {
            ArriveFrom.top => Offset(0, -travel * left),
            ArriveFrom.right => Offset(travel * left, 0),
            ArriveFrom.nowhere => Offset.zero,
          };
          return Transform.translate(offset: offset, child: child);
        },
        child: widget.child,
      ),
    );
  }
}
