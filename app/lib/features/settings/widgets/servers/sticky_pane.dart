/// A pane that stays in view beside a longer neighbour while the page
/// scrolls, and a reporter of a child's height (HUM-414).
library;

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

import '../../../../core/ui/ui.dart';

/// The list beside a long detail view: it stays in view while the page
/// scrolls, down to the end of the frame, and goes with the frame after.
///
/// It stands in the frame's stack, not in the row, so a pointer reaches it
/// wherever it moves to. The shift comes from the scroll offset and the
/// place of the frame in the scrolled content, which scrolling does not
/// change.
class StickyPane extends StatefulWidget {
  /// The list in [frame], [width] wide.
  const StickyPane({
    required this.frame,
    required this.width,
    required this.child,
    super.key,
  });

  /// The frame of list and detail, whose height bounds the shift.
  final GlobalKey frame;

  /// The width of the list.
  final double width;

  /// The list.
  final Widget child;

  @override
  State<StickyPane> createState() => _StickyPaneState();
}

class _StickyPaneState extends State<StickyPane> {
  ScrollPosition? _position;
  final GlobalKey _pane = GlobalKey(debugLabel: 'server-list-pane');

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final ScrollPosition? position = Scrollable.maybeOf(context)?.position;
    if (position != _position) {
      _position?.removeListener(_scrolled);
      _position = position?..addListener(_scrolled);
    }
  }

  @override
  void dispose() {
    _position?.removeListener(_scrolled);
    super.dispose();
  }

  void _scrolled() {
    if (mounted) {
      setState(() {});
    }
  }

  double _shift() {
    final ScrollPosition? position = _position;
    final RenderObject? frame = widget.frame.currentContext?.findRenderObject();
    final RenderObject? pane = _pane.currentContext?.findRenderObject();
    if (position == null ||
        frame is! RenderBox ||
        pane is! RenderBox ||
        !frame.attached ||
        !frame.hasSize ||
        !pane.hasSize) {
      return 0;
    }
    final RenderAbstractViewport? viewport = RenderAbstractViewport.maybeOf(
      frame,
    );
    if (viewport == null) {
      return 0;
    }
    final double top = viewport.getOffsetToReveal(frame, 0).offset;
    final double room = frame.size.height - pane.size.height;
    if (room <= 0) {
      return 0;
    }
    return (position.pixels - top + HSpace.x2).clamp(0, room).toDouble();
  }

  @override
  Widget build(BuildContext context) => Positioned(
    left: 0,
    top: _shift(),
    width: widget.width,
    child: KeyedSubtree(key: _pane, child: widget.child),
  );
}

/// Reports the height of [child] after each layout that changed it.
class SizeReport extends SingleChildRenderObjectWidget {
  /// Reports to [onHeight].
  const SizeReport({required this.onHeight, required super.child, super.key});

  /// Told the new height, after the frame.
  final ValueChanged<double> onHeight;

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _RenderSizeReport(onHeight);

  @override
  void updateRenderObject(BuildContext context, RenderObject renderObject) =>
      (renderObject as _RenderSizeReport).onHeight = onHeight;
}

class _RenderSizeReport extends RenderProxyBox {
  _RenderSizeReport(this.onHeight);

  ValueChanged<double> onHeight;
  double? _reported;

  @override
  void performLayout() {
    super.performLayout();
    final double height = size.height;
    if (height != _reported) {
      _reported = height;
      WidgetsBinding.instance.addPostFrameCallback(
        (Duration _) => onHeight(height),
      );
    }
  }
}
