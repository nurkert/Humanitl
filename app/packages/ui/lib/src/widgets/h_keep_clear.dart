import 'package:flutter/foundation.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/widgets.dart';

/// A region that nothing floating may cover (HUM-300).
///
/// The floating terminal of the application lies over the sections, and some
/// of what lies there must stay clickable whatever floats: the queue with its
/// rows and its notices, the bar with Allow and Block, the notices of the
/// shell, an open sheet, a modal. Each of them wraps itself in this widget,
/// and whatever floats asks [rectsIn] where they are and stays out of them.
///
/// **The region says where it is, the floater decides.** No list of screens
/// or panes lives in the floater; a control that must stay free marks itself
/// where it is built, and the rule follows it to every screen it appears on.
///
/// **Only what is shown counts.** The shell keeps every section built in an
/// `IndexedStack`, which tells the hidden ones through `Visibility.of`; a
/// region in a hidden section is not on the screen and blocks nothing.
///
/// **An empty region counts as nothing.** A notice column without a notice
/// has no height; a rectangle of zero height still "overlaps" every rectangle
/// across it in [Rect.overlaps], so [rectsIn] drops it.
///
/// **A floater never keeps clear from itself.** A region inside the floater
/// (below [HKeepClearExempt]) counts as nothing: the notices of the floating
/// terminal stand in its own window, and a window that gave way to them took
/// them along, found its place free again in the next frame and came back,
/// several times a second (HUM-352).
///
/// **A region may hold nothing to keep free right now.** [enabled] false
/// counts it as nothing, like a hidden one: the bar with Allow and Block
/// while no request waits (HUM-352). When it turns true, [changes] tells the
/// floater, and the floater gives way again.
///
/// The region itself draws nothing, takes no pointer and no focus; it is a
/// proxy around [child].
class HKeepClear extends StatelessWidget {
  /// Marks [child] as a region nothing may float over, while [enabled].
  const HKeepClear({required this.child, this.enabled = true, super.key});

  /// What must stay free.
  final Widget child;

  /// Whether [child] holds something that must stay free right now.
  final bool enabled;

  static final Set<_RenderKeepClear> _regions = <_RenderKeepClear>{};

  static final ValueNotifier<int> _changes = ValueNotifier<int>(0);

  static bool _scheduled = false;

  /// Changes whenever a region came, went, changed its size or its
  /// visibility. Bumped after the frame in which that happened, never during
  /// it, so a listener may rebuild at once.
  ///
  /// **A region that only moves, at the same size, does not bump it.** No
  /// render object learns its own position without a walk up the tree, and
  /// doing that every frame for every region would cost more than the case
  /// is worth: the regions marked today change size whenever the layout
  /// around them moves them (a dragged pane boundary, a resized window). A
  /// floater that must also follow pure moves asks [rectsIn] again on its
  /// own rebuilds.
  static ValueListenable<int> get changes => _changes;

  /// Every shown, laid-out, non-empty region, in the coordinates of [target].
  ///
  /// Read from the last layout of each region: a floater that asks while it
  /// is built gets the regions as the last frame left them, and [changes]
  /// tells it when that was not the final word.
  static List<Rect> rectsIn(RenderBox target) {
    final List<Rect> rects = <Rect>[];
    if (!target.attached) {
      return rects;
    }
    for (final _RenderKeepClear region in _regions) {
      if (!region.active || !region.attached || !region.hasSize) {
        continue;
      }
      final Size size = region.size;
      if (size.isEmpty) {
        continue;
      }
      final Matrix4 transform = region.getTransformTo(target);
      final Rect rect = MatrixUtils.transformRect(
        transform,
        Offset.zero & size,
      );
      rects.add(rect);
    }
    return rects;
  }

  static void _changed() {
    if (_scheduled) {
      return;
    }
    _scheduled = true;
    SchedulerBinding.instance
      ..addPostFrameCallback((Duration _) {
        _scheduled = false;
        _changes.value++;
      })
      ..ensureVisualUpdate();
  }

  @override
  Widget build(BuildContext context) => _KeepClearBox(
    active:
        enabled && Visibility.of(context) && !HKeepClearExempt.covers(context),
    child: child,
  );
}

class _KeepClearBox extends SingleChildRenderObjectWidget {
  const _KeepClearBox({required this.active, required super.child});

  final bool active;

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _RenderKeepClear(active);

  @override
  void updateRenderObject(BuildContext context, _RenderKeepClear renderObject) {
    renderObject.active = active;
  }
}

class _RenderKeepClear extends RenderProxyBox {
  _RenderKeepClear(this._active);

  bool _active;

  bool get active => _active;

  set active(bool value) {
    if (value == _active) {
      return;
    }
    _active = value;
    HKeepClear._changed();
  }

  Size? _lastSize;

  @override
  void attach(PipelineOwner owner) {
    super.attach(owner);
    HKeepClear._regions.add(this);
    HKeepClear._changed();
  }

  @override
  void detach() {
    HKeepClear._regions.remove(this);
    HKeepClear._changed();
    super.detach();
  }

  @override
  void performLayout() {
    super.performLayout();
    // Nur eine neue Größe meldet sich: Ein Bereich, der bei jedem Frame neu
    // gelegt wird, ohne sich zu ändern, ließe den Schwebenden sonst in jedem
    // Frame neu rechnen.
    if (size != _lastSize) {
      _lastSize = size;
      HKeepClear._changed();
    }
  }
}

/// The floater itself: every [HKeepClear] below it counts as nothing
/// (HUM-352).
///
/// Whatever floats and asks [HKeepClear.rectsIn] wraps itself in this. Its
/// own regions move with it; were they counted, it would give way to itself.
class HKeepClearExempt extends InheritedWidget {
  /// Exempts the regions in [child].
  const HKeepClearExempt({required super.child, super.key});

  /// Whether [context] lies inside a floater.
  static bool covers(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<HKeepClearExempt>() != null;

  @override
  bool updateShouldNotify(HKeepClearExempt oldWidget) => false;
}
