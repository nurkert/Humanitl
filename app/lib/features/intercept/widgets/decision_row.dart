/// The line of the action bar that carries both decisions (HUM-307).
///
/// Allow and Block stand on one line at every width the inspector can have
/// and at every common text scale. Before HUM-307 the bar switched to a
/// column below a fixed width of 800 px, measured with the square glyphs of
/// the test font; with the real fonts the controls fit long before that, and
/// Block sat one line below Allow in almost every window.
///
/// The line now measures what it carries and gives way in a fixed order, so
/// that what goes first is decided here and not left to a `Wrap`:
///
/// 1. All controls at their natural size; Allow stays left, Block right,
///    and every pixel the controls leave over goes to the distance between
///    them.
/// 2. Where the pane is narrower, the distance shrinks down to
///    [DecisionRow.minGap]; nothing else gives way before it.
/// 3. "Edit + Allow" leaves the line. It is the one control that decides
///    nothing, and `E` still opens the editor.
/// 4. The valve shrinks; its label ends in an ellipsis, never below
///    [DecisionRow.minDecisionWidth] and never below what its own content
///    needs.
/// 5. Only when even that does not fit -- twice the text scale in the
///    narrowest pane -- Block moves under Allow, right aligned, as
///    `docs/UX.md` 6 allows for scaled text.
///
/// The note slot always stands directly beside Block and counts towards the
/// distance: it is empty at rest, and a click that misses Block lands on
/// nothing.
library;

import 'dart:math' as math;

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

/// The places of [DecisionRow].
enum DecisionRowSlot {
  /// The release valve.
  valve,

  /// "Edit + Allow", beside the valve.
  edit,

  /// The note slot, beside Block.
  note,

  /// The block control, at the right edge.
  block,
}

/// Lays out valve, edit control, note slot and Block on one line.
///
/// See the library comment for the order in which the line gives way.
class DecisionRow extends StatefulWidget {
  /// Creates the line.
  const DecisionRow({
    required this.valve,
    required this.note,
    required this.block,
    required this.gap,
    required this.minGap,
    required this.minDecisionWidth,
    required this.editSpacing,
    required this.noteSpacing,
    required this.lineSpacing,
    this.edit,
    super.key,
  });

  /// The release valve.
  final Widget valve;

  /// "Edit + Allow", or null when there is no editor.
  final Widget? edit;

  /// The note slot.
  final Widget note;

  /// The block control.
  final Widget block;

  /// The distance the line asks for in its maximum intrinsic width.
  ///
  /// The layout itself guarantees only [minGap] and hands everything beyond
  /// it to the distance; this value only tells a parent that measures the
  /// line how wide it would like to be.
  final double gap;

  /// The distance below which the line rather drops the edit control.
  final double minGap;

  /// The narrowest the valve gets before Block moves to a line of its own.
  final double minDecisionWidth;

  /// The space between valve and edit control.
  final double editSpacing;

  /// The space between note slot and Block.
  final double noteSpacing;

  /// The space between the lines once Block stands under Allow.
  final double lineSpacing;

  @override
  State<DecisionRow> createState() => _DecisionRowState();
}

class _DecisionRowState extends State<DecisionRow> {
  /// The gate in front of the edit control's focus.
  ///
  /// The layout decides whether the control stands on the line, and a
  /// control that is not painted must not keep the focus or a stop in the
  /// focus order: `Enter` in the next frame would otherwise open the editor
  /// from a button nobody sees (HUM-307, review). The gate is switched in the
  /// same layout that drops the control, directly on the node and without a
  /// rebuild, so the focus leaves before the next key event is handled.
  final FocusNode _editGate = FocusNode(
    debugLabel: 'DecisionRow edit gate',
    canRequestFocus: false,
    skipTraversal: true,
  );

  void _laidOut(bool editHidden) {
    final bool reachable = !editHidden;
    if (_editGate.descendantsAreFocusable != reachable) {
      _editGate.descendantsAreFocusable = reachable;
    }
    if (_editGate.descendantsAreTraversable != reachable) {
      _editGate.descendantsAreTraversable = reachable;
    }
  }

  @override
  void dispose() {
    _editGate.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final Widget? edit = widget.edit;
    return _DecisionRowLayout(
      valve: widget.valve,
      // No `descendantsAreFocusable` here: the widget leaves the node's own
      // values alone, and `_laidOut` keeps them.
      edit: edit == null ? null : Focus(focusNode: _editGate, child: edit),
      note: widget.note,
      block: widget.block,
      gap: widget.gap,
      minGap: widget.minGap,
      minDecisionWidth: widget.minDecisionWidth,
      editSpacing: widget.editSpacing,
      noteSpacing: widget.noteSpacing,
      lineSpacing: widget.lineSpacing,
      onLaidOut: _laidOut,
    );
  }
}

class _DecisionRowLayout
    extends SlottedMultiChildRenderObjectWidget<DecisionRowSlot, RenderBox> {
  const _DecisionRowLayout({
    required this.valve,
    required this.edit,
    required this.note,
    required this.block,
    required this.gap,
    required this.minGap,
    required this.minDecisionWidth,
    required this.editSpacing,
    required this.noteSpacing,
    required this.lineSpacing,
    required this.onLaidOut,
  });

  final Widget valve;
  final Widget? edit;
  final Widget note;
  final Widget block;
  final double gap;
  final double minGap;
  final double minDecisionWidth;
  final double editSpacing;
  final double noteSpacing;
  final double lineSpacing;
  final ValueChanged<bool> onLaidOut;

  @override
  Iterable<DecisionRowSlot> get slots => DecisionRowSlot.values;

  @override
  Widget? childForSlot(DecisionRowSlot slot) => switch (slot) {
    DecisionRowSlot.valve => valve,
    DecisionRowSlot.edit => edit,
    DecisionRowSlot.note => note,
    DecisionRowSlot.block => block,
  };

  @override
  RenderDecisionRow createRenderObject(BuildContext context) =>
      RenderDecisionRow(
        gap: gap,
        minGap: minGap,
        minDecisionWidth: minDecisionWidth,
        editSpacing: editSpacing,
        noteSpacing: noteSpacing,
        lineSpacing: lineSpacing,
        onLaidOut: onLaidOut,
      );

  @override
  void updateRenderObject(
    BuildContext context,
    RenderDecisionRow renderObject,
  ) {
    renderObject
      ..gap = gap
      ..minGap = minGap
      ..minDecisionWidth = minDecisionWidth
      ..editSpacing = editSpacing
      ..noteSpacing = noteSpacing
      ..lineSpacing = lineSpacing
      ..onLaidOut = onLaidOut;
  }
}

/// Where each control of [RenderDecisionRow] goes, computed once for a
/// layout and once for a dry layout.
class _Plan {
  _Plan(this.size, this.offsets, this.editHidden);

  final Size size;
  final Map<DecisionRowSlot, Offset> offsets;
  final bool editHidden;
}

/// The render object of [DecisionRow].
class RenderDecisionRow extends RenderBox
    with SlottedContainerRenderObjectMixin<DecisionRowSlot, RenderBox> {
  /// Creates the render object.
  RenderDecisionRow({
    required this._gap,
    required this._minGap,
    required this._minDecisionWidth,
    required this._editSpacing,
    required this._noteSpacing,
    required this._lineSpacing,
    required this.onLaidOut,
  });

  /// Told after every layout whether the edit control was left out.
  ValueChanged<bool> onLaidOut;

  double _gap;

  /// See [DecisionRow.gap].
  set gap(double value) => _set(_gap, value, () => _gap = value);

  double _minGap;

  /// See [DecisionRow.minGap].
  set minGap(double value) => _set(_minGap, value, () => _minGap = value);

  double _minDecisionWidth;

  /// See [DecisionRow.minDecisionWidth].
  set minDecisionWidth(double value) =>
      _set(_minDecisionWidth, value, () => _minDecisionWidth = value);

  double _editSpacing;

  /// See [DecisionRow.editSpacing].
  set editSpacing(double value) =>
      _set(_editSpacing, value, () => _editSpacing = value);

  double _noteSpacing;

  /// See [DecisionRow.noteSpacing].
  set noteSpacing(double value) =>
      _set(_noteSpacing, value, () => _noteSpacing = value);

  double _lineSpacing;

  /// See [DecisionRow.lineSpacing].
  set lineSpacing(double value) =>
      _set(_lineSpacing, value, () => _lineSpacing = value);

  void _set(double old, double value, VoidCallback assign) {
    if (old != value) {
      assign();
      markNeedsLayout();
    }
  }

  /// Whether the last layout left the edit control out.
  bool get editHidden => _editHidden;
  bool _editHidden = false;

  RenderBox? get _valve => childForSlot(DecisionRowSlot.valve);
  RenderBox? get _edit => childForSlot(DecisionRowSlot.edit);
  RenderBox? get _note => childForSlot(DecisionRowSlot.note);
  RenderBox? get _block => childForSlot(DecisionRowSlot.block);

  /// The children that are painted, hit and read out, in paint order.
  Iterable<RenderBox> get _shown => <RenderBox?>[
    _valve,
    if (!_editHidden) _edit,
    _note,
    _block,
  ].whereType<RenderBox>();

  @override
  void setupParentData(RenderObject child) {
    if (child.parentData is! BoxParentData) {
      child.parentData = BoxParentData();
    }
  }

  /// The line for [constraints], with [layoutChild] sizing each child: the
  /// real layout or the dry one.
  _Plan _plan(BoxConstraints constraints, ChildLayouter layoutChild) {
    assert(
      constraints.hasBoundedWidth,
      'The decisions need a bounded width to know what fits.',
    );
    final double width = constraints.maxWidth;
    Size measure(RenderBox? child, double maxWidth) => child == null
        ? Size.zero
        : layoutChild(child, BoxConstraints(maxWidth: math.max(0, maxWidth)));

    final Size note = measure(_note, width);
    Size block = measure(_block, width);
    Size valve = measure(_valve, width);
    final Size edit = measure(_edit, width);
    final bool hasEdit = _edit != null;
    final double right = note.width + _noteSpacing + block.width;
    // The free space between the Allow side and the note slot that keeps
    // Allow and Block [distance] apart; the note slot stands inside the
    // distance, and some air always stays between the two sides.
    double free(double distance) =>
        math.max(distance - note.width - _noteSpacing, _editSpacing);
    final double withEdit = hasEdit ? _editSpacing + edit.width : 0;

    bool editHidden = false;
    bool stacked = false;
    if (valve.width + withEdit + free(_minGap) + right <= width) {
      // Steps 1 and 2: everything at its natural size.
    } else if (valve.width + free(_minGap) + right <= width) {
      // Step 3.
      editHidden = hasEdit;
    } else if (width - free(_minGap) - right >= _narrowestValve()) {
      // Step 4.
      editHidden = hasEdit;
      valve = measure(_valve, width - free(_minGap) - right);
    } else {
      stacked = true;
    }

    final Map<DecisionRowSlot, Offset> offsets = <DecisionRowSlot, Offset>{};
    if (!stacked) {
      final double height = <double>[
        valve.height,
        if (hasEdit && !editHidden) edit.height,
        note.height,
        block.height,
      ].reduce(math.max);
      double centred(Size size) => (height - size.height) / 2;
      offsets[DecisionRowSlot.valve] = Offset(0, centred(valve));
      offsets[DecisionRowSlot.edit] = Offset(
        valve.width + _editSpacing,
        centred(edit),
      );
      offsets[DecisionRowSlot.block] = Offset(
        width - block.width,
        centred(block),
      );
      offsets[DecisionRowSlot.note] = Offset(width - right, centred(note));
      return _Plan(
        constraints.constrain(Size(width, height)),
        offsets,
        editHidden,
      );
    }

    // Step 5: the valve and the edit control on the first line, the edit
    // control on a line of its own when it does not fit beside the valve,
    // and the note slot with Block on the last line, right aligned.
    double y = 0;
    final bool editBeside =
        hasEdit && valve.width + _editSpacing + edit.width <= width;
    final double first = editBeside
        ? math.max(valve.height, edit.height)
        : valve.height;
    offsets[DecisionRowSlot.valve] = Offset(0, (first - valve.height) / 2);
    if (editBeside) {
      offsets[DecisionRowSlot.edit] = Offset(
        valve.width + _editSpacing,
        (first - edit.height) / 2,
      );
    }
    y += first;
    if (hasEdit && !editBeside) {
      y += _lineSpacing;
      offsets[DecisionRowSlot.edit] = Offset(0, y);
      y += edit.height;
    }
    y += _lineSpacing;
    if (right > width) {
      block = measure(_block, width - note.width - _noteSpacing);
    }
    final double last = math.max(note.height, block.height);
    offsets[DecisionRowSlot.block] = Offset(
      width - block.width,
      y + (last - block.height) / 2,
    );
    offsets[DecisionRowSlot.note] = Offset(
      width - block.width - _noteSpacing - note.width,
      y + (last - note.height) / 2,
    );
    y += last;
    return _Plan(constraints.constrain(Size(width, y)), offsets, false);
  }

  /// The narrowest the valve may get: [DecisionRow.minDecisionWidth], or
  /// more when its own content needs more.
  ///
  /// At twice the text scale the padding, the shortcut and the chevron of
  /// the valve alone are wider than 120 px; squeezed below its minimum
  /// intrinsic width, its inner row would overflow instead of shortening
  /// the label.
  double _narrowestValve() {
    final RenderBox? valve = _valve;
    return valve == null
        ? _minDecisionWidth
        : math.max(
            _minDecisionWidth,
            valve.getMinIntrinsicWidth(double.infinity),
          );
  }

  @override
  void performLayout() {
    final _Plan plan = _plan(constraints, ChildLayoutHelper.layoutChild);
    for (final DecisionRowSlot slot in DecisionRowSlot.values) {
      final RenderBox? child = childForSlot(slot);
      if (child != null) {
        (child.parentData! as BoxParentData).offset =
            plan.offsets[slot] ?? Offset.zero;
      }
    }
    size = plan.size;
    if (_editHidden != plan.editHidden) {
      _editHidden = plan.editHidden;
      markNeedsSemanticsUpdate();
    }
    onLaidOut(_editHidden);
  }

  @override
  Size computeDryLayout(covariant BoxConstraints constraints) =>
      _plan(constraints, ChildLayoutHelper.dryLayoutChild).size;

  @override
  double computeMinIntrinsicWidth(double height) => <double>[
    for (final RenderBox child in children) child.getMinIntrinsicWidth(height),
  ].fold(0, math.max);

  @override
  double computeMaxIntrinsicWidth(double height) {
    double sum = 0;
    for (final RenderBox child in children) {
      sum += child.getMaxIntrinsicWidth(height);
    }
    return sum + _editSpacing + _gap + _noteSpacing;
  }

  @override
  double computeMinIntrinsicHeight(double width) =>
      getDryLayout(BoxConstraints(maxWidth: width)).height;

  @override
  double computeMaxIntrinsicHeight(double width) =>
      computeMinIntrinsicHeight(width);

  @override
  void paint(PaintingContext context, Offset offset) {
    for (final RenderBox child in _shown) {
      context.paintChild(
        child,
        offset + (child.parentData! as BoxParentData).offset,
      );
    }
  }

  @override
  bool hitTestChildren(BoxHitTestResult result, {required Offset position}) {
    for (final RenderBox child in _shown.toList().reversed) {
      final Offset childOffset = (child.parentData! as BoxParentData).offset;
      final bool hit = result.addWithPaintOffset(
        offset: childOffset,
        position: position,
        hitTest: (BoxHitTestResult result, Offset transformed) =>
            child.hitTest(result, position: transformed),
      );
      if (hit) {
        return true;
      }
    }
    return false;
  }

  @override
  void visitChildrenForSemantics(RenderObjectVisitor visitor) =>
      _shown.forEach(visitor);
}

/// A label that [DecisionRow] may shorten down to nothing.
///
/// A one-line text with an ellipsis reports its full width as its minimum
/// intrinsic width, and the valve would then claim to need its whole label.
/// Wrapped in this, the label claims no minimum, and the valve's minimum is
/// what really cannot shrink: padding, glyph, shortcut and chevron. The
/// layout of the label itself is unchanged.
class ShrinkableLabel extends SingleChildRenderObjectWidget {
  /// Wraps [child].
  const ShrinkableLabel({required Widget super.child, super.key});

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _RenderShrinkableLabel();
}

class _RenderShrinkableLabel extends RenderProxyBox {
  @override
  double computeMinIntrinsicWidth(double height) => 0;
}
