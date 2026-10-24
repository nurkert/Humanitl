/// Side-by-side panes with draggable splitters, on plain Flutter: no
/// `ResizablePane` of a component library (HUM-020 Fallstricke). Candidate
/// for `packages/ui` as `HResizable` (handoff).
library;

import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
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

/// [widths] with every seam on a whole pixel: the boundaries between panes are
/// rounded, and each pane takes the difference, so the sum stays the same.
///
/// Eine Naht auf einem halben Pixel wäre eine Haarlinie, die über zwei
/// Pixelspalten verschmiert und blasser aussieht als jede andere Linie des
/// Systems (HUM-286).
List<double> snapPaneWidths(List<double> widths) {
  final List<double> snapped = <double>[];
  double edge = 0;
  double sum = 0;
  for (int i = 0; i < widths.length; i++) {
    sum += widths[i];
    final double next = i == widths.length - 1 ? sum : sum.roundToDouble();
    snapped.add(math.max(0, next - edge));
    edge = next;
  }
  return snapped;
}

/// A row of panes separated by splitters the pointer can drag.
///
/// The widget owns no state: [ratios] come in and [onRatiosChanged] reports
/// every drag, so the owner can persist them.
///
/// Jeder Splitter ist ein Fokusstopp, und die Pfeiltasten verschieben ihn um
/// [HSize.splitterStep]: jeder Zeigerweg hat eine Taste, und ein Ziehen ist
/// keine Ausnahme (`docs/UX.md` 5.1). Ohne das ist die Aufteilung des
/// Bildschirms ohne Maus nicht erreichbar.
class HResizablePanes extends StatefulWidget {
  /// Creates panes; [children], [ratios] and [minWidths] have equal length.
  const HResizablePanes({
    required this.children,
    required this.ratios,
    required this.minWidths,
    required this.onRatiosChanged,
    this.splitterWidth = HSize.splitter,
    this.splitterSemanticsLabel,
    super.key,
  }) : assert(
         children.length == ratios.length && ratios.length == minWidths.length,
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
  ///
  /// [HSize.splitter] by default; no literal, and odd on purpose so the
  /// hairline lands on a whole pixel (`docs/UX.md` 2.1). The handle lies over
  /// the seam and takes no width from the panes: between two panes there is
  /// exactly one [HSize.border] (HUM-286).
  final double splitterWidth;

  /// Screen-reader label of every splitter, already localised.
  ///
  /// Null leaves the splitter unnamed; a caller that has a string for it
  /// passes it in, because this layer holds no user-visible text.
  final String? splitterSemanticsLabel;

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
        // Zwischen zwei Panes liegt genau eine Haarlinie und kein Streifen:
        // Die Panes stoßen an die Naht, und der Griff liegt als
        // unsichtbare Fläche über ihr, halb links, halb rechts. Ein Griff,
        // der sich seine sieben Pixel aus der Zeile nähme, ließe neben jeder
        // Naht einen Streifen im Hintergrund der Seite stehen; eine Leiste,
        // die bündig an ihrem Pane enden soll, stünde dann eingerückt in
        // ihm (HUM-286, `docs/UX.md` "Layout grid").
        const double seam = HSize.border;
        final double available = math.max(
          0,
          constraints.maxWidth - seam * (n - 1),
        );
        final List<double> widths = snapPaneWidths(
          resolvePaneWidths(available, widget.ratios, widget.minWidths),
        );
        final List<Widget> grips = <Widget>[];
        double x = 0;
        for (int i = 0; i < n - 1; i++) {
          x += widths[i];
          final int index = i;
          grips.add(
            Positioned(
              top: 0,
              bottom: 0,
              left: x - (widget.splitterWidth - seam) / 2,
              width: widget.splitterWidth,
              child: FocusTraversalOrder(
                order: NumericFocusOrder(2.0 * index + 1),
                child: _Splitter(
                  width: widget.splitterWidth,
                  active: _dragging == index,
                  color: _dragging == index
                      ? tokens.colors.accent
                      : tokens.colors.line,
                  semanticsLabel: widget.splitterSemanticsLabel,
                  onStart: () => setState(() => _dragging = index),
                  onUpdate: (double delta) => _drag(index, delta, available),
                  onEnd: () => setState(() => _dragging = null),
                ),
              ),
            ),
          );
          x += seam;
        }
        // Die Griffe liegen im Baum nach den Panes, damit sie über deren
        // Rand den Zeiger bekommen. Die Tastatur geht trotzdem Pane,
        // Splitter, Pane: Die Reihenfolge steht ausdrücklich da und nicht im
        // Zufall der Geometrie; innerhalb eines Panes bleibt die Lesefolge
        // (`docs/UX.md` 5.1).
        return FocusTraversalGroup(
          policy: OrderedTraversalPolicy(),
          child: Stack(
            children: <Widget>[
              Positioned.fill(
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    for (int i = 0; i < n; i++) ...<Widget>[
                      SizedBox(
                        width: widths[i],
                        child: FocusTraversalOrder(
                          order: NumericFocusOrder(2.0 * i),
                          child: widget.children[i],
                        ),
                      ),
                      if (i < n - 1) const SizedBox(width: seam),
                    ],
                  ],
                ),
              ),
              ...grips,
            ],
          ),
        );
      },
    );
  }
}

/// Verschiebt einen fokussierten Splitter um einen Schritt.
class _NudgeIntent extends Intent {
  const _NudgeIntent(this.direction);

  /// -1 nach links, 1 nach rechts.
  final double direction;
}

class _Splitter extends StatefulWidget {
  const _Splitter({
    required this.width,
    required this.active,
    required this.color,
    required this.semanticsLabel,
    required this.onStart,
    required this.onUpdate,
    required this.onEnd,
  });

  final double width;
  final bool active;
  final Color color;
  final String? semanticsLabel;
  final VoidCallback onStart;
  final ValueChanged<double> onUpdate;
  final VoidCallback onEnd;

  @override
  State<_Splitter> createState() => _SplitterState();
}

class _SplitterState extends State<_Splitter> {
  bool _focused = false;

  /// Ein Tastendruck ist ein vollständiger Zug: anfassen, verschieben,
  /// loslassen. Ohne das Loslassen bliebe der Splitter im Zustand „wird
  /// gezogen" stehen, sobald jemand einmal eine Pfeiltaste gedrückt hat.
  void _nudge(double direction) {
    widget
      ..onStart()
      ..onUpdate(direction * HSize.splitterStep)
      ..onEnd();
  }

  @override
  Widget build(BuildContext context) {
    final bool marked = widget.active || _focused;
    return Semantics(
      label: widget.semanticsLabel,
      child: FocusableActionDetector(
        mouseCursor: SystemMouseCursors.resizeColumn,
        onFocusChange: (bool value) => setState(() => _focused = value),
        shortcuts: <ShortcutActivator, Intent>{
          const SingleActivator(LogicalKeyboardKey.arrowLeft):
              const _NudgeIntent(-1),
          const SingleActivator(LogicalKeyboardKey.arrowRight):
              const _NudgeIntent(1),
        },
        actions: <Type, Action<Intent>>{
          _NudgeIntent: CallbackAction<_NudgeIntent>(
            onInvoke: (_NudgeIntent intent) {
              _nudge(intent.direction);
              return null;
            },
          ),
        },
        child: HFocusRing.inline(
          visible: _focused,
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            dragStartBehavior: DragStartBehavior.down,
            onHorizontalDragStart: (DragStartDetails _) => widget.onStart(),
            onHorizontalDragUpdate: (DragUpdateDetails d) =>
                widget.onUpdate(d.delta.dx),
            onHorizontalDragEnd: (DragEndDetails _) => widget.onEnd(),
            onHorizontalDragCancel: widget.onEnd,
            child: SizedBox(
              width: widget.width,
              child: Center(
                child: SizedBox(
                  // Kein Literal: die gezogene Linie ist doppelt so breit wie
                  // die ruhende Haarlinie (`docs/UX.md` 2.1).
                  width: marked ? HSize.splitterActive : HSize.hairline,
                  // Die volle Höhe: Ohne sie bekäme die Linie im `Center`
                  // die Höhe null und wäre nie zu sehen gewesen; die Naht
                  // zeigten bis HUM-286 nur die Kanten der Panes daneben.
                  height: double.infinity,
                  child: ColoredBox(color: widget.color),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
