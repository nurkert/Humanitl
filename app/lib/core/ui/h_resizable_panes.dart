/// Side-by-side panes with draggable splitters, on plain Flutter: no
/// `ResizablePane` of a component library (HUM-020 Fallstricke). Candidate
/// for `packages/ui` as `HResizable` (handoff).
library;

import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';

import 'ui.dart';

/// Widths for [ratios] of [total], each at least its entry of [minWidths]
/// where the total allows it.
///
/// Panes below their minimum are raised to it and the difference is taken
/// from the others, in proportion to what they have above their own minimum.
/// When even the minimums do not fit, every pane gets its share of the
/// minimums instead; nothing ever goes negative.
List<double> resolvePaneWidths(
  double total,
  List<double> ratios,
  List<double> minWidths,
) {
  assert(ratios.length == minWidths.length, 'one minimum per pane');
  final int n = ratios.length;
  if (n == 0) {
    return const <double>[];
  }
  final double minSum = minWidths.fold(0, (double a, double b) => a + b);
  if (total <= minSum) {
    return <double>[
      for (final double min in minWidths)
        minSum == 0 ? total / n : total * min / minSum,
    ];
  }
  final double ratioSum = ratios.fold(0, (double a, double b) => a + b);
  final List<double> widths = <double>[
    for (final double r in ratios)
      ratioSum == 0 ? total / n : total * r / ratioSum,
  ];
  // Raise the short ones; take the deficit from the slack of the others.
  for (int round = 0; round < n; round++) {
    double deficit = 0;
    final List<int> short = <int>[];
    for (int i = 0; i < n; i++) {
      if (widths[i] < minWidths[i]) {
        deficit += minWidths[i] - widths[i];
        widths[i] = minWidths[i];
        short.add(i);
      }
    }
    if (deficit == 0) {
      break;
    }
    double slack = 0;
    for (int i = 0; i < n; i++) {
      if (!short.contains(i)) {
        slack += math.max(0, widths[i] - minWidths[i]);
      }
    }
    if (slack <= 0) {
      break;
    }
    for (int i = 0; i < n; i++) {
      if (!short.contains(i)) {
        final double own = math.max(0, widths[i] - minWidths[i]);
        widths[i] -= deficit * own / slack;
      }
    }
  }
  return widths;
}

/// A row of panes separated by splitters the pointer can drag.
///
/// The widget owns no state: [ratios] come in and [onRatiosChanged] reports
/// every drag, so the owner can persist them.
class HResizablePanes extends StatefulWidget {
  /// Creates panes; [children], [ratios] and [minWidths] have equal length.
  const HResizablePanes({
    required this.children,
    required this.ratios,
    required this.minWidths,
    required this.onRatiosChanged,
    this.splitterWidth = 7,
    super.key,
  }) : assert(
         children.length == ratios.length &&
             ratios.length == minWidths.length,
         'one ratio and one minimum per child',
       );

  /// The panes, left to right.
  final List<Widget> children;

  /// Relative widths; any positive numbers, normalised by their sum.
  final List<double> ratios;

  /// Minimum width of each pane.
  final List<double> minWidths;

  /// Invoked with the new ratios (summing to 1) while a splitter is dragged.
  final ValueChanged<List<double>> onRatiosChanged;

  /// Width of the drag handle; a 1 px hairline sits in its middle.
  final double splitterWidth;

  @override
  State<HResizablePanes> createState() => _HResizablePanesState();
}

class _HResizablePanesState extends State<HResizablePanes> {
  int? _dragging;

  void _drag(int splitter, double delta, double available) {
    final List<double> widths = resolvePaneWidths(
      available,
      widget.ratios,
      widget.minWidths,
    );
    final int left = splitter;
    final int right = splitter + 1;
    // Keep both neighbours at or above their minimum; the other panes stay.
    final double maxGrowLeft = widths[right] - widget.minWidths[right];
    final double maxShrinkLeft = widths[left] - widget.minWidths[left];
    final double applied = delta.clamp(-maxShrinkLeft, maxGrowLeft);
    if (applied == 0) {
      return;
    }
    widths[left] += applied;
    widths[right] -= applied;
    widget.onRatiosChanged(<double>[
      for (final double w in widths) w / available,
    ]);
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final int n = widget.children.length;
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final double available = math.max(
          0,
          constraints.maxWidth - widget.splitterWidth * (n - 1),
        );
        final List<double> widths = resolvePaneWidths(
          available,
          widget.ratios,
          widget.minWidths,
        );
        return Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            for (int i = 0; i < n; i++) ...<Widget>[
              SizedBox(width: widths[i], child: widget.children[i]),
              if (i < n - 1)
                _Splitter(
                  width: widget.splitterWidth,
                  active: _dragging == i,
                  color: _dragging == i
                      ? tokens.colors.accent
                      : tokens.colors.line,
                  onStart: () => setState(() => _dragging = i),
                  onUpdate: (double delta) => _drag(i, delta, available),
                  onEnd: () => setState(() => _dragging = null),
                ),
            ],
          ],
        );
      },
    );
  }
}

class _Splitter extends StatelessWidget {
  const _Splitter({
    required this.width,
    required this.active,
    required this.color,
    required this.onStart,
    required this.onUpdate,
    required this.onEnd,
  });

  final double width;
  final bool active;
  final Color color;
  final VoidCallback onStart;
  final ValueChanged<double> onUpdate;
  final VoidCallback onEnd;

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.resizeColumn,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        dragStartBehavior: DragStartBehavior.down,
        onHorizontalDragStart: (DragStartDetails _) => onStart(),
        onHorizontalDragUpdate: (DragUpdateDetails d) => onUpdate(d.delta.dx),
        onHorizontalDragEnd: (DragEndDetails _) => onEnd(),
        onHorizontalDragCancel: onEnd,
        child: SizedBox(
          width: width,
          child: Center(
            child: SizedBox(
              width: active ? 2 : HSize.hairline,
              child: ColoredBox(color: color),
            ),
          ),
        ),
      ),
    );
  }
}
