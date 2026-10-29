import 'package:flutter/widgets.dart';

/// The focus scope of a layer above the work: a modal, a sheet, a menu
/// (HUM-303).
///
/// **The layer takes the keyboard when it opens, and gives it back when it
/// closes.** `FocusScope(autofocus: true)` is not enough for that: autofocus
/// yields as soon as anything in the surrounding scope already has the
/// focus, and in this application something always does -- the shell, the
/// queue, or the terminal of the agent. A context menu opened over the
/// terminal then stood on the screen while every key, `Esc` included, still
/// went to the agent. This scope therefore asks for the focus itself once it
/// is built.
///
/// **Between the request and that build lies one frame, and a key can arrive
/// in it.** An `Enter` typed right after "Create rule" was clicked would
/// still reach the queue and allow the held request; an `Esc` right after
/// the right click would still cancel the agent. Whoever opens a layer
/// therefore calls [claimKeyboard] in the same callback, before the layer is
/// even built: it takes the keyboard from its holder at once and parks it on
/// the scope around that holder, where no binding of the work listens. The
/// layer then takes it from there, and on closing it gives it back to the
/// holder [claimKeyboard] took it from -- or, without a claim, to whoever had
/// it when the layer was built.
///
/// The hand-back is explicit, because parking the keyboard clears the memory
/// of the scope that would otherwise restore it. It happens only when the
/// layer still held the keyboard at the end (a layer somebody already left
/// keeps its hands off), only when no newer layer is open (that one has the
/// keyboard and will hand it on), and a layer that closes while a newer one
/// was opened from inside it passes its own earlier holder on to that newer
/// layer. So the palette that runs "Allow all" closes, the modal it opened
/// takes the keyboard, and closing the modal returns it to the queue.
///
/// The terminal relies on the scope too: it takes the keyboard only from a
/// node in its own scope or one around it, and the scope of a layer is never
/// one of those (`TerminalPane._mayTakeKeyboard`).
class HLayerScope extends StatefulWidget {
  /// Creates the scope around [child].
  const HLayerScope({required this.child, super.key});

  /// The content of the layer.
  final Widget child;

  /// The holder [claimKeyboard] took the keyboard from, until a layer is
  /// built and takes it over.
  static FocusNode? _claimed;

  /// Every open layer, oldest first.
  static final List<_HLayerScopeState> _open = <_HLayerScopeState>[];

  /// Takes the keyboard from whoever holds it, now, because a layer is about
  /// to open.
  ///
  /// Call it in the callback that opens the layer, before the layer is
  /// built. If no layer is built by the end of the next frame, the keyboard
  /// goes back to where it was.
  static void claimKeyboard() {
    final FocusNode? holder = FocusManager.instance.primaryFocus;
    if (holder == null || _claimed != null) {
      return;
    }
    _claimed = holder;
    // Flutter applies this in a microtask, and microtasks run before the
    // next event from the platform; a key cannot slip in between.
    holder.unfocus();
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      final FocusNode? unclaimed = _claimed;
      _claimed = null;
      if (unclaimed != null && _usable(unclaimed)) {
        unclaimed.requestFocus();
      }
    });
    WidgetsBinding.instance.scheduleFrame();
  }

  /// Whether a layer has claimed the keyboard and does not hold it yet:
  /// claimed and not built, or built and still waiting for its request.
  ///
  /// Whoever hands the keyboard back to itself whenever it lands on a scope
  /// around it -- the Intercept screen does, and the terminal when it comes
  /// back into its pane -- asks this first, or it would undo the claim and
  /// let the next key decide, or take the keyboard away from the layer the
  /// moment before it arrives.
  static bool get keyboardClaimed =>
      _claimed != null || _open.any((_HLayerScopeState l) => l._opening);

  /// Whether the keyboard is inside a layer right now.
  ///
  /// Asked by whoever is about to open a layer over another one -- the
  /// command palette over the settings sheet -- and must not hand the
  /// keyboard to something behind both when it closes. Works in release
  /// builds too, unlike a `debugLabel`.
  static bool holdsFocus() {
    final BuildContext? context = FocusManager.instance.primaryFocus?.context;
    return context != null &&
        context.findAncestorStateOfType<_HLayerScopeState>() != null;
  }

  /// Whether [node] is still on the screen and can take the keyboard.
  static bool _usable(FocusNode node) {
    final BuildContext? where = node.context;
    return where != null && where.mounted && node.canRequestFocus;
  }

  @override
  State<HLayerScope> createState() => _HLayerScopeState();
}

class _HLayerScopeState extends State<HLayerScope> {
  final FocusScopeNode _scope = FocusScopeNode(debugLabel: 'layer');

  /// Who gets the keyboard back when the layer closes.
  FocusNode? _previous;

  /// The open layer [_previous] stands in, if any.
  _HLayerScopeState? _previousLayer;

  /// Whether the layer held the keyboard at the last change of focus.
  ///
  /// Read in [dispose]: by then the scope is already detached, and Flutter
  /// does not notify a detached node any more, so this is the last word on
  /// whether the keyboard was still here when the layer went.
  bool _held = false;

  /// True from the build until the layer first held the keyboard.
  bool _opening = true;

  @override
  void initState() {
    super.initState();
    final FocusNode? previous =
        HLayerScope._claimed ?? FocusManager.instance.primaryFocus;
    HLayerScope._claimed = null;
    _previous = previous;
    for (final _HLayerScopeState layer in HLayerScope._open) {
      if (previous != null &&
          (previous == layer._scope ||
              previous.ancestors.contains(layer._scope))) {
        _previousLayer = layer;
      }
    }
    HLayerScope._open.add(this);
    _scope.addListener(_focusChanged);
    // After the frame that builds the scope: before it is attached a request
    // goes nowhere, and `autofocus` yields to whoever holds the focus. The
    // frame in between is covered by [HLayerScope.claimKeyboard].
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (!mounted) {
        return;
      }
      if (_scope.hasFocus) {
        _opening = false;
      } else if (_scope.canRequestFocus && Visibility.of(context)) {
        _scope.requestFocus();
      } else {
        // Hidden: it will not get the keyboard, and nobody may wait for it.
        _opening = false;
      }
    });
  }

  void _focusChanged() {
    _held = _scope.hasFocus;
    if (_held) {
      _opening = false;
    }
  }

  @override
  void dispose() {
    _scope.removeListener(_focusChanged);
    final int index = HLayerScope._open.indexOf(this);
    HLayerScope._open.remove(this);
    // A newer layer opened from inside this one inherits the holder this
    // one would have given the keyboard back to.
    for (final _HLayerScopeState layer in HLayerScope._open) {
      if (identical(layer._previousLayer, this)) {
        layer
          .._previous = _previous
          .._previousLayer = _previousLayer;
      }
    }
    final bool newerOpen = index >= 0 && index < HLayerScope._open.length;
    final FocusNode? previous = _previous;
    if (_held &&
        !newerOpen &&
        previous != null &&
        HLayerScope._usable(previous)) {
      previous.requestFocus();
    }
    _scope.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      FocusScope(node: _scope, autofocus: true, child: widget.child);
}
