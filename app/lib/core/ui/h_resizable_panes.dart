/// Panes with draggable splitters, side by side or stacked, on plain Flutter:
/// no `ResizablePane` of a component library (HUM-020 Fallstricke). Candidate
/// for `packages/ui` as `HResizable` (handoff).
///
/// Seit HUM-305 auch übereinander: Der Sandbox-Abschnitt teilt Terminal und
/// Reiter mit demselben Widget, das die Intercept-Spalten teilt. Ein zweites
/// Widget für die andere Richtung hätte die Naht, das Raster und die Tasten
/// ein zweites Mal gebaut, und die beiden wären auseinandergelaufen.
library;

import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/scheduler.dart';
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

/// A row (or, with [axis] vertical, a column) of panes separated by
/// splitters the pointer can drag.
///
/// The widget owns no state: [ratios] come in and [onRatiosChanged] reports
/// every drag, so the owner can show them; [onDragEnd] says when a drag is
/// over, so the owner can persist them once and not on every pixel.
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
    this.axis = Axis.horizontal,
    this.onDragEnd,
    this.onReset,
    this.splitterWidth = HSize.splitter,
    this.splitterSemanticsLabel,
    super.key,
  }) : assert(
         children.length == ratios.length && ratios.length == minWidths.length,
         'one ratio and one minimum per child',
       );

  /// The panes, left to right, or top to bottom on the vertical [axis].
  final List<Widget> children;

  /// Relative extents along [axis]; any positive numbers, normalised by
  /// their sum.
  final List<double> ratios;

  /// Minimum extent of each pane along [axis]: a width on the horizontal
  /// axis, a height on the vertical one.
  final List<double> minWidths;

  /// Invoked with the new ratios (summing to 1) while a splitter is dragged.
  final ValueChanged<List<double>> onRatiosChanged;

  /// The direction the panes follow each other in. Horizontal by default:
  /// columns side by side, splitters standing upright.
  final Axis axis;

  /// Invoked once when a drag ends, and after every step of the keyboard.
  ///
  /// The place to write the ratios down: [onRatiosChanged] fires for every
  /// pixel of a drag, and a file written that often would be a file written
  /// for nothing (HUM-305).
  final VoidCallback? onDragEnd;

  /// Invoked on a double click on a splitter: the owner puts its default
  /// split back. Null leaves the double click without effect.
  final VoidCallback? onReset;

  /// Thickness of the drag handle across [axis]; a 1 px hairline sits in its
  /// middle.
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

  /// The ratios this widget last reported, until the owner hands them back.
  ///
  /// Zwei Ereignisse in einem Frame (zwei Pfeiltasten, zwei Zeigerbewegungen)
  /// rechneten sonst beide von denselben alten [HResizablePanes.ratios] aus,
  /// und der zweite Schritt überschriebe den ersten, statt ihn fortzusetzen
  /// (HUM-305).
  List<double>? _reported;

  @override
  void didUpdateWidget(HResizablePanes oldWidget) {
    super.didUpdateWidget(oldWidget);
    _reported = null;
  }

  void _drag(int splitter, double delta, double available) {
    final List<double> widths = resolvePaneWidths(
      available,
      _reported ?? widget.ratios,
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
    final List<double> ratios = <double>[
      for (final double w in widths) w / available,
    ];
    _reported = ratios;
    // Nur bis zum nächsten Frame: Ein Besitzer, der die Werte nicht
    // zurückgibt (eine feste Aufteilung), behält seine Aufteilung.
    SchedulerBinding.instance.addPostFrameCallback((Duration _) {
      if (identical(_reported, ratios)) {
        _reported = null;
      }
    });
    widget.onRatiosChanged(ratios);
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
        final bool vertical = widget.axis == Axis.vertical;
        final double available = math.max(
          0,
          (vertical ? constraints.maxHeight : constraints.maxWidth) -
              seam * (n - 1),
        );
        final List<double> widths = snapPaneWidths(
          resolvePaneWidths(available, widget.ratios, widget.minWidths),
        );
        final List<Widget> grips = <Widget>[];
        double x = 0;
        for (int i = 0; i < n - 1; i++) {
          x += widths[i];
          final int index = i;
          final double start = x - (widget.splitterWidth - seam) / 2;
          grips.add(
            Positioned(
              top: vertical ? start : 0,
              bottom: vertical ? null : 0,
              left: vertical ? 0 : start,
              right: vertical ? 0 : null,
              width: vertical ? null : widget.splitterWidth,
              height: vertical ? widget.splitterWidth : null,
              child: FocusTraversalOrder(
                order: NumericFocusOrder(2.0 * index + 1),
                child: _Splitter(
                  axis: widget.axis,
                  width: widget.splitterWidth,
                  active: _dragging == index,
                  color: _dragging == index
                      ? tokens.colors.accent
                      : tokens.colors.line,
                  semanticsLabel: widget.splitterSemanticsLabel,
                  onStart: () => setState(() => _dragging = index),
                  onUpdate: (double delta) => _drag(index, delta, available),
                  onEnd: () {
                    setState(() => _dragging = null);
                    widget.onDragEnd?.call();
                  },
                  onReset: widget.onReset,
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
                child: Flex(
                  direction: widget.axis,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    for (int i = 0; i < n; i++) ...<Widget>[
                      SizedBox(
                        width: vertical ? null : widths[i],
                        height: vertical ? widths[i] : null,
                        child: FocusTraversalOrder(
                          order: NumericFocusOrder(2.0 * i),
                          child: widget.children[i],
                        ),
                      ),
                      if (i < n - 1)
                        SizedBox(
                          width: vertical ? null : seam,
                          height: vertical ? seam : null,
                        ),
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

  /// -1 nach links oder oben, 1 nach rechts oder unten.
  final double direction;
}

class _Splitter extends StatefulWidget {
  const _Splitter({
    required this.axis,
    required this.width,
    required this.active,
    required this.color,
    required this.semanticsLabel,
    required this.onStart,
    required this.onUpdate,
    required this.onEnd,
    required this.onReset,
  });

  final Axis axis;
  final double width;
  final bool active;
  final Color color;
  final String? semanticsLabel;
  final VoidCallback onStart;
  final ValueChanged<double> onUpdate;
  final VoidCallback onEnd;
  final VoidCallback? onReset;

  @override
  State<_Splitter> createState() => _SplitterState();
}

class _SplitterState extends State<_Splitter> {
  bool _focused = false;

  /// Wo und wann der Zeiger zuletzt aufgesetzt hat.
  Offset? _downAt;

  /// Wann der letzte Klick ohne Zug endete, oder null.
  Duration? _lastClick;

  /// Wie weit der Griff seit dem letzten Aufsetzen gezogen wurde.
  ///
  /// Ein Zug ist kein Klick, auch wenn er kürzer als [kDoubleTapSlop] war:
  /// Sonst setzte ein Zug um 80 px und ein Klick gleich danach die Aufteilung
  /// zurück. Gezählt wird der Weg, den der Erkenner meldet, und nicht sein
  /// Start: Steht er allein in der Arena, gewinnt er schon beim Aufsetzen und
  /// meldet mit [DragStartBehavior.down] einen Start für jeden Klick.
  double _travel = 0;

  void _dragUpdate(double delta) {
    _travel += delta.abs();
    if (_travel > kTouchSlop) {
      _lastClick = null;
    }
    widget.onUpdate(delta);
  }

  /// Ein Doppelklick stellt die Vorgabe des Bildschirms wieder her (HUM-305).
  ///
  /// Von Hand gezählt und nicht über `GestureDetector.onDoubleTap`: Dessen
  /// Erkenner steht mit dem Ziehen in derselben Arena und lässt nach jedem
  /// Zug einen Zeitgeber zurück, der in jedem Test, der eine Naht zieht, als
  /// offener Zeitgeber auffiele. Ein Klick ist ein Aufsetzen und Loslassen
  /// ohne Bewegung über die Schwelle; zwei davon innerhalb von
  /// [kDoubleTapTimeout] sind ein Doppelklick. Ein Zug dazwischen zählt neu.
  void _pointerDown(PointerDownEvent event) {
    _downAt = event.position;
    _travel = 0;
  }

  void _pointerUp(PointerUpEvent event) {
    final Offset? down = _downAt;
    final bool dragged = _travel > kTouchSlop;
    _downAt = null;
    _travel = 0;
    final VoidCallback? reset = widget.onReset;
    if (dragged ||
        reset == null ||
        down == null ||
        (event.position - down).distance > kDoubleTapSlop) {
      _lastClick = null;
      return;
    }
    final Duration? last = _lastClick;
    if (last != null && event.timeStamp - last <= kDoubleTapTimeout) {
      _lastClick = null;
      reset();
      return;
    }
    _lastClick = event.timeStamp;
  }

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
    final bool vertical = widget.axis == Axis.vertical;
    // Die Linie liegt quer zur Achse: aufrecht zwischen Spalten, liegend
    // zwischen Zeilen. Kein Literal: gezogen ist sie doppelt so dick wie die
    // ruhende Haarlinie (`docs/UX.md` 2.1).
    final double line = marked ? HSize.splitterActive : HSize.hairline;
    return Semantics(
      label: widget.semanticsLabel,
      child: FocusableActionDetector(
        mouseCursor: vertical
            ? SystemMouseCursors.resizeRow
            : SystemMouseCursors.resizeColumn,
        onFocusChange: (bool value) => setState(() => _focused = value),
        // Nur die Pfeile der eigenen Achse. Sie kommen ohnehin nur an,
        // solange der Splitter selbst den Fokus hat; ein Terminal mit Fokus
        // behält jeden Pfeil (HUM-305).
        shortcuts: <ShortcutActivator, Intent>{
          SingleActivator(
            vertical
                ? LogicalKeyboardKey.arrowUp
                : LogicalKeyboardKey.arrowLeft,
          ): const _NudgeIntent(
            -1,
          ),
          SingleActivator(
            vertical
                ? LogicalKeyboardKey.arrowDown
                : LogicalKeyboardKey.arrowRight,
          ): const _NudgeIntent(
            1,
          ),
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
          child: Listener(
            onPointerDown: _pointerDown,
            onPointerUp: _pointerUp,
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              dragStartBehavior: DragStartBehavior.down,
              onHorizontalDragStart: vertical
                  ? null
                  : (DragStartDetails _) => widget.onStart(),
              onHorizontalDragUpdate: vertical
                  ? null
                  : (DragUpdateDetails d) => _dragUpdate(d.delta.dx),
              onHorizontalDragEnd: vertical
                  ? null
                  : (DragEndDetails _) => widget.onEnd(),
              onHorizontalDragCancel: vertical ? null : widget.onEnd,
              onVerticalDragStart: vertical
                  ? (DragStartDetails _) => widget.onStart()
                  : null,
              onVerticalDragUpdate: vertical
                  ? (DragUpdateDetails d) => _dragUpdate(d.delta.dy)
                  : null,
              onVerticalDragEnd: vertical
                  ? (DragEndDetails _) => widget.onEnd()
                  : null,
              onVerticalDragCancel: vertical ? widget.onEnd : null,
              child: SizedBox(
                width: vertical ? null : widget.width,
                height: vertical ? widget.width : null,
                child: Center(
                  // Die volle Länge: Ohne sie bekäme die Linie im `Center` die
                  // Länge null und wäre nie zu sehen gewesen; die Naht zeigten
                  // bis HUM-286 nur die Kanten der Panes daneben.
                  child: SizedBox(
                    width: vertical ? double.infinity : line,
                    height: vertical ? line : double.infinity,
                    child: ColoredBox(color: widget.color),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
