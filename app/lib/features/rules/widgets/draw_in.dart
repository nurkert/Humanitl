/// A row that draws itself once, when it is new.
///
/// The one arrival this screen has: "this rule exists now" (`docs/UX.md` 2.1,
/// second question). It runs for [HMotion.ruleDraw] on [HMotion.enter] and
/// then leaves the tree, so a list of two hundred rules carries two hundred
/// rows and no wrappers (`docs/UX.md` 7).
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';

/// Draws [child] in once and then gets out of the way.
class DrawIn extends StatefulWidget {
  /// Wraps [child]. With [animate] false the child is shown at once, which is
  /// what a list does for the rules it already had.
  const DrawIn({required this.child, this.animate = true, super.key});

  /// The row.
  final Widget child;

  /// Whether this is a new row.
  final bool animate;

  @override
  State<DrawIn> createState() => _DrawInState();
}

class _DrawInState extends State<DrawIn> with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: HMotion.ruleDraw,
    value: widget.animate ? 0 : 1,
  );
  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _controller,
    curve: HMotion.enter,
  );
  bool _done = false;

  @override
  void initState() {
    super.initState();
    _controller.addStatusListener(_finish);
    if (widget.animate) {
      _controller.forward();
    } else {
      _done = true;
    }
  }

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
    // Reduced motion keeps the feedback and drops the distance: the row does
    // not grow into place, it appears at full height and fades in
    // (`docs/UX.md` 2.10).
    final Widget faded = FadeTransition(opacity: _curve, child: widget.child);
    if (HReducedMotion.of(context)) {
      return faded;
    }
    return SizeTransition(
      sizeFactor: _curve,
      alignment: const Alignment(-1, -1),
      child: faded,
    );
  }
}
