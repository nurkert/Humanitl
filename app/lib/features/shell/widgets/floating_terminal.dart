/// The terminal of the agent as a small window over the sections, picture
/// in picture (HUM-300).
///
/// The owner watches the agent while deciding its requests. The window is the
/// third host of the one terminal (`terminal_placement.dart`): it builds the
/// same `SandboxTerminal` as the pane and the whole window, attaches to the
/// same session and shows the same scrollback. While it floats, the pane of
/// the Sandbox section stays empty. The window itself is drawn in
/// `floating_terminal_window.dart`; this file decides where it stands.
///
/// **What it never covers.** Every region marked with `HKeepClear` -- the
/// queue with its rows and notices, the bar with Allow and Block, the notices
/// of the shell, an open sheet, a modal -- stays free. The window snaps to the
/// corner the person chose; when something to keep clear lies there, it
/// takes the next free corner, then the smallest size, and when no corner is
/// free it shrinks to a slim title strip that finds a place of its own
/// (`placeTerminalStrip`, `floating_terminal_strip.dart`, HUM-315); only
/// without room even for that it steps back until there is room. Two
/// exceptions while a hand holds it: a resize never grows into such a region,
/// a drag may pass over one, and both give way again when they are let go.
///
/// **The keyboard.** No focus scope of its own and no `HLayerScope`: the
/// window is part of the work, not a layer above it, so the terminal in it
/// takes the keyboard under the same rule as in its pane (`TerminalPane`,
/// HUM-303) -- on hover, by `Ctrl+Backquote`, by a click -- and leaves it
/// where it is otherwise. Opening the window from another section leaves the
/// keyboard with the queue.
///
/// **The end.** When the agent ends, the window shows the end as the pane
/// would for [terminalFloatLinger], then puts the terminal back into its pane
/// by itself. A new agent within that time keeps it open. The countdown runs
/// wherever the terminal stands: a window that comes back from the whole
/// window after the time is over closes at once.
///
/// **The Sandbox page.** While it shows, the terminal stands in its pane there
/// and this layer draws nothing (`terminalHostProvider`, HUM-315); the choice
/// to float stays and takes effect again on leaving the page. An agent that
/// ends while the page shows docks the terminal at once: the pane shows the
/// end itself, and leaving the page must not float a terminal whose session
/// is over.
library;

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart' show kTouchSlop;
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/terminal_float.dart';
import '../../../core/ui/terminal_placement.dart';
import '../../../core/ui/ui.dart';
import '../../sandbox/providers/sandbox_status_provider.dart';
import 'floating_terminal_strip.dart';
import 'floating_terminal_window.dart';

/// How long a floating terminal stays after its agent ended, showing the end
/// as the pane would show it, before it closes itself (HUM-300).
///
/// Not a motion token and therefore not in `HMotion`: no animation runs for
/// it, it is the time a person gets to read the end. It lives here and not in
/// `core/ui`, where every duration comes from the tokens (`docs/UX.md` 2.1).
const Duration terminalFloatLinger = Duration(seconds: 4);

/// The sections, and the floating terminal over them while it floats.
class FloatingTerminalLayer extends ConsumerStatefulWidget {
  /// Lays the floating terminal over [child].
  const FloatingTerminalLayer({
    required this.child,
    required this.onBack,
    super.key,
  });

  /// The sections. They keep their full size whether the window floats.
  final Widget child;

  /// Puts the terminal back into its pane and shows the Sandbox section.
  final VoidCallback onBack;

  @override
  ConsumerState<FloatingTerminalLayer> createState() =>
      _FloatingTerminalLayerState();
}

class _FloatingTerminalLayerState extends ConsumerState<FloatingTerminalLayer> {
  /// Where the window's top left stands while it is dragged; null otherwise.
  Offset? _dragOrigin;

  /// The size while the grip is dragged; null otherwise.
  Size? _resizing;

  /// The last place, region and corner the layout gave the window: where a
  /// drag or a resize starts from.
  Rect? _shown;
  Rect _region = Rect.zero;
  FloatCorner _shownCorner = FloatCorner.bottomRight;

  /// The render box of the window's region, from its last layout: what a
  /// resize measures the regions to keep clear against.
  RenderBox? _regionBox;

  /// Counts down after the agent ended, wherever the terminal stands.
  Timer? _linger;

  /// Whether the agent ended and the countdown after it is over: a window
  /// that floats again from now on closes at once.
  bool _endShown = false;

  static final ProviderListenable<bool> _agentRunning = sandboxStatusProvider
      .select(
        (AsyncValue<SandboxStatus> status) =>
            status.value?.agentRunning ?? false,
      );

  @override
  void initState() {
    super.initState();
    HKeepClear.changes.addListener(_regionsChanged);
    ref
      ..listenManual<bool>(_agentRunning, _agentChanged)
      ..listenManual<TerminalPlacement>(terminalPlacementProvider, (
        TerminalPlacement? _,
        TerminalPlacement next,
      ) {
        if (next == TerminalPlacement.floating) {
          _closeIfEnded();
        }
      })
      ..listenManual<int>(terminalFocusRequestProvider, (int? _, int _) {
        _focusAsked();
      });
  }

  /// Somebody asked for the terminal with the keyboard -- ``Ctrl+` ``, "Go
  /// to terminal" of the empty queue -- while this layer shows only the
  /// strip, or nothing: there is no emulator here to take it (HUM-315,
  /// review). The same as a click on the strip without room: the terminal
  /// goes back into its pane on the Sandbox page, and the request is asked
  /// again one frame later, once the pane stands.
  void _focusAsked() {
    if (!mounted || !_shownHere || _shown != null) {
      return;
    }
    widget.onBack();
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (mounted) {
        ref.read(terminalFocusRequestProvider.notifier).request();
      }
    });
  }

  @override
  void dispose() {
    HKeepClear.changes.removeListener(_regionsChanged);
    _linger?.cancel();
    super.dispose();
  }

  /// Whether the person chose to let the terminal float, whatever page shows.
  bool get _floating =>
      ref.read(terminalPlacementProvider) == TerminalPlacement.floating;

  /// Whether this layer shows the terminal now: it floats and the Sandbox
  /// page does not show (HUM-315).
  bool get _shownHere =>
      ref.read(terminalHostProvider) == TerminalPlacement.floating;

  /// A region to keep clear came, went or changed its size: the place is
  /// computed anew.
  void _regionsChanged() {
    if (mounted && _shownHere) {
      setState(() {});
    }
  }

  /// The end of an agent starts the countdown, a new one stops it. The
  /// countdown does not ask where the terminal stands: it may be over the
  /// whole window now and float again later (review round 1).
  void _agentChanged(bool? previous, bool running) {
    _linger?.cancel();
    _linger = null;
    if (running) {
      _endShown = false;
      return;
    }
    if (previous != true) {
      return;
    }
    // Auf der Sandbox-Seite zeigt das Pane das Ende selbst; das Fenster
    // schwebte erst beim Verlassen wieder, über einer Sitzung, die vorbei ist
    // (HUM-315). Also gleich zurück, nach der Änderung, die das meldete.
    if (_floating && ref.read(sandboxPageShownProvider)) {
      _endShown = true;
      _closeIfEnded();
      return;
    }
    _linger = Timer(terminalFloatLinger, () {
      _linger = null;
      if (!mounted || ref.read(_agentRunning)) {
        return;
      }
      _endShown = true;
      _closeIfEnded();
    });
  }

  /// Puts a floating terminal back once its agent's end has been shown.
  /// After the change of placement that called it, not inside it.
  void _closeIfEnded() {
    if (!_endShown) {
      return;
    }
    scheduleMicrotask(() {
      if (mounted && _endShown && _floating && !ref.read(_agentRunning)) {
        ref.read(terminalPlacementProvider.notifier).dock();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final bool floating =
        ref.watch(terminalHostProvider) == TerminalPlacement.floating;
    final TerminalFloatGeometry geometry = ref.watch(terminalFloatProvider);
    return Stack(
      fit: StackFit.expand,
      children: <Widget>[
        widget.child,
        if (floating)
          LayoutBuilder(
            builder: (BuildContext context, BoxConstraints constraints) =>
                _place(context, constraints, geometry),
          ),
      ],
    );
  }

  /// The window at its place, else the strip at its place, or nothing while
  /// not even the strip fits.
  Widget _place(
    BuildContext context,
    BoxConstraints constraints,
    TerminalFloatGeometry geometry,
  ) {
    final HTokens tokens = HTheme.of(context);
    final double inset = tokens.spacing.x3;
    final Rect region = Offset.zero & constraints.biggest;
    _region = region;
    final RenderObject? box = context.findRenderObject();
    _regionBox = box is RenderBox ? box : null;
    final Rect? rect =
        _interactiveRect(region, inset) ?? _avoiding(region, geometry, inset);
    _shown = rect;
    if (rect == null) {
      return _strip(region, geometry, inset);
    }
    return Stack(
      children: <Widget>[
        Positioned.fromRect(
          rect: rect,
          child: FloatingTerminalWindow(
            corner: _shownCorner,
            onDragStart: _dragStarted,
            onDragUpdate: _dragged,
            onDragEnd: _dragEnded,
            onResizeStart: _resizeStarted,
            onResizeUpdate: _resized,
            onResizeEnd: _resizeEnded,
            onBack: widget.onBack,
            onClose: ref.read(terminalPlacementProvider.notifier).dock,
          ),
        ),
      ],
    );
  }

  /// The title strip in the first free corner from the chosen one, or
  /// nothing while not even the strip fits (HUM-315).
  Widget _strip(Rect region, TerminalFloatGeometry geometry, double inset) {
    final FloatPlacement? placement = placeTerminalStrip(
      region: region,
      corner: geometry.corner,
      size: HTheme.of(context).sizes.terminalStrip,
      keepClear: _keepClear(),
      inset: inset,
    );
    if (placement == null) {
      return const SizedBox.shrink();
    }
    return Stack(
      children: <Widget>[
        Positioned.fromRect(
          rect: placement.rect,
          child: LiveFloatingTerminalStrip(onOpen: _openFromStrip),
        ),
      ],
    );
  }

  /// The strip was pressed: the window in the next free corner, measured
  /// against the regions as they stand now, or, with none free, the terminal
  /// back in its pane on the Sandbox page (HUM-315). The second is what is
  /// left: the person asked to see the terminal, and a pane on a page that
  /// does not show would hide it altogether.
  void _openFromStrip() {
    final double inset = HTheme.of(context).spacing.x3;
    final Rect? rect = _avoiding(
      _region,
      ref.read(terminalFloatProvider),
      inset,
    );
    if (rect != null) {
      setState(() {});
      return;
    }
    widget.onBack();
  }

  /// The regions to keep clear, in the coordinates of the window's region.
  List<Rect> _keepClear() {
    final RenderBox? box = _regionBox;
    return box == null ? const <Rect>[] : HKeepClear.rectsIn(box);
  }

  /// The place the rule picks among the regions to keep clear.
  Rect? _avoiding(Rect region, TerminalFloatGeometry geometry, double inset) {
    final FloatPlacement? placement = placeFloatingTerminal(
      region: region,
      geometry: geometry,
      keepClear: _keepClear(),
      inset: inset,
    );
    if (placement == null) {
      return null;
    }
    _shownCorner = placement.corner;
    return placement.rect;
  }

  /// The place while a hand moves the window or its grip: it follows the
  /// pointer until it is let go. A drag may pass over a region to keep clear
  /// (the hand is busy with the window, and the place is decided on release);
  /// a resize never grows into one ([_resized]).
  Rect? _interactiveRect(Rect region, double inset) {
    final Offset? origin = _dragOrigin;
    final Rect? shown = _shown;
    if (origin != null && shown != null) {
      final Offset clamped = Offset(
        origin.dx.clamp(0, math.max(0, region.width - shown.width)).toDouble(),
        origin.dy
            .clamp(0, math.max(0, region.height - shown.height))
            .toDouble(),
      );
      return clamped & shown.size;
    }
    final Size? size = _resizing;
    if (size != null) {
      return terminalFloatRectAt(
        region,
        _shownCorner,
        clampTerminalFloatSize(size, region: region.size),
        inset,
      );
    }
    return null;
  }

  /// Where the drag began, to tell a drag from a click.
  Offset? _dragFrom;

  void _dragStarted() {
    final Rect? shown = _shown;
    if (shown != null) {
      _dragFrom = shown.topLeft;
      setState(() => _dragOrigin = shown.topLeft);
    }
  }

  void _dragged(Offset delta) {
    final Offset? origin = _dragOrigin;
    if (origin != null) {
      setState(() => _dragOrigin = origin + delta);
    }
  }

  /// Snaps to the corner of the quadrant the window was let go in, and keeps
  /// that corner.
  void _dragEnded() {
    // Ohne Zug kein Einrasten: Ein Klick auf die Titelleiste oder einen ihrer
    // Knöpfe endet als abgebrochener Zug, oder, steht der Zug allein in der
    // Arena, als Zug ohne Weg. Die Ecke, in die das Fenster gerade ausweicht,
    // würde sonst als gewählte gespeichert (Review-Runde 1).
    final Offset? origin = _dragOrigin;
    final Offset? from = _dragFrom;
    _dragFrom = null;
    if (origin == null) {
      return;
    }
    if (from != null && (origin - from).distance < kTouchSlop) {
      setState(() => _dragOrigin = null);
      return;
    }
    // Aus dem Zug selbst und nicht aus dem letzten Bild: Mehrere Bewegungen
    // und das Loslassen können vor dem nächsten Frame ankommen.
    final Rect? shown = _interactiveRect(_region, 0) ?? _shown;
    if (shown != null) {
      ref
          .read(terminalFloatProvider.notifier)
          .snapTo(FloatCorner.nearest(_region, shown.center));
    }
    setState(() => _dragOrigin = null);
  }

  void _resizeStarted() {
    final Rect? shown = _shown;
    if (shown != null) {
      setState(() => _resizing = shown.size);
    }
  }

  /// The grip sits in the corner across from the one the window holds to, so
  /// moving it away from that corner makes the window larger. Each direction
  /// is taken only while the window does not grow into a region to keep
  /// clear: the hand cannot pull it over Allow and Block.
  void _resized(Offset delta) {
    final Size? size = _resizing;
    if (size == null) {
      return;
    }
    final double dx = _shownCorner.left ? delta.dx : -delta.dx;
    final double dy = _shownCorner.top ? delta.dy : -delta.dy;
    final List<Rect> keepClear = _keepClear();
    final double inset = HTheme.of(context).spacing.x3;
    bool free(Size candidate) {
      final Rect rect = terminalFloatRectAt(
        _region,
        _shownCorner,
        clampTerminalFloatSize(candidate, region: _region.size),
        inset,
      );
      return !keepClear.any((Rect zone) => zone.overlaps(rect));
    }

    Size next = size;
    final Size wider = Size(next.width + dx, next.height);
    if (dx <= 0 || free(wider)) {
      next = wider;
    }
    final Size taller = Size(next.width, next.height + dy);
    if (dy <= 0 || free(taller)) {
      next = taller;
    }
    setState(() => _resizing = next);
  }

  void _resizeEnded() {
    final Size? size = _resizing;
    if (size != null) {
      ref
          .read(terminalFloatProvider.notifier)
          .resize(clampTerminalFloatSize(size, region: _region.size));
    }
    setState(() => _resizing = null);
  }
}
