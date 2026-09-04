/// The left pane: the queue of held requests (HUM-020, HUM-029).
///
/// The list is an `AnimatedList` fed from `visibleQueueFlowsProvider`, and the
/// difference between two snapshots is computed over item keys, never over
/// positions (HUM-020 Fallstricke). Requests that share a registrable domain
/// stand under one header, so a burst of twelve reads as one thing.
///
/// Nothing moves under a reading eye (`docs/UX.md` 2.8): while the pointer is
/// in the pane, while the last keyboard navigation is younger than
/// [HMotion.freezeAfterKey] and while more than one request is selected, an
/// arrival does not enter the list -- it only raises the pill over the top
/// row. The frozen view lives in this `State` and nowhere else; pointer
/// movement never reaches the provider graph (`docs/UX.md` 7 and 8).
library;

import 'dart:async';
import 'dart:math' as math;

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/announce.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../list_diff.dart';
import '../providers/flows.dart';
import '../providers/held_groups.dart';
import '../providers/queue_freeze.dart';
import '../providers/selection.dart';
import '../queue_items.dart';
import 'group_header_row.dart';
import 'new_arrivals_pill.dart';
import 'queue_row.dart';

/// How often a burst of arrivals is said out loud, at most.
///
/// Fifteen arrivals in twenty seconds would otherwise be fifteen full URLs in
/// somebody's ear (`docs/UX.md` 6). Policy, not motion: it belongs beside
/// `queueExitWindow` and not in the motion table.
const Duration arrivalAnnounceWindow = Duration(seconds: 2);

/// The queue pane.
class QueuePane extends ConsumerStatefulWidget {
  /// Creates the pane.
  const QueuePane({super.key});

  @override
  ConsumerState<QueuePane> createState() => _QueuePaneState();
}

class _QueuePaneState extends ConsumerState<QueuePane> {
  final GlobalKey<AnimatedListState> _listKey = GlobalKey<AnimatedListState>(
    debugLabel: 'intercept-queue',
  );

  /// The whole visible queue, as the provider last delivered it.
  List<Flow> _all = const <Flow>[];

  /// The flows the list is allowed to show; an arrival that is held back is
  /// not in here.
  final Set<FlowId> _admitted = <FlowId>{};

  /// The arrivals that wait for the reading eye to look away.
  final Set<FlowId> _waiting = <FlowId>{};

  /// Arrivals that have not been said out loud yet.
  final List<Flow> _unsaid = <Flow>[];

  /// The lines as they stand.
  List<QueueItem> _items = const <QueueItem>[];

  /// Where in its own duration an arriving line starts, so that several
  /// arrivals in one frame come in one after the other (`docs/UX.md` 2.2).
  final Map<String, double> _stagger = <String, double>{};

  bool _pointerInside = false;
  bool _keyNavRecent = false;

  /// Counts arrivals, so the badge in the head can pulse once for each burst.
  int _arrivals = 0;

  Timer? _keyTimer;
  Timer? _pointerTimer;
  Timer? _announceTimer;

  @override
  void initState() {
    super.initState();
    _all = ref.read(visibleQueueFlowsProvider).flows;
    _admitted.addAll(<FlowId>[for (final Flow flow in _all) flow.id]);
    _items = _lines();
  }

  @override
  void dispose() {
    _keyTimer?.cancel();
    _pointerTimer?.cancel();
    _announceTimer?.cancel();
    super.dispose();
  }

  /// True while the order may not change under the eye.
  bool get _frozen =>
      _pointerInside || _keyNavRecent || ref.read(selectionProvider).length > 1;

  /// The lines of the flows that are admitted right now.
  List<QueueItem> _lines() {
    final List<Flow> visible = <Flow>[
      for (final Flow flow in _all)
        if (_admitted.contains(flow.id)) flow,
    ];
    return queueItems(
      groupFlows(visible),
      ref.read(expandedGroupsProvider.notifier).isOpen,
    );
  }

  /// Takes a new snapshot in, holding arrivals back while the queue is frozen.
  void _sync(List<Flow> next) {
    if (!mounted) {
      return;
    }
    _all = next;
    final Set<FlowId> ids = <FlowId>{for (final Flow flow in next) flow.id};
    _admitted.retainWhere(ids.contains);
    _waiting.retainWhere(ids.contains);
    final List<Flow> arrivals = <Flow>[
      for (final Flow flow in next)
        if (!_admitted.contains(flow.id) && !_waiting.contains(flow.id)) flow,
    ];
    if (_frozen) {
      _waiting.addAll(<FlowId>[for (final Flow flow in arrivals) flow.id]);
    } else {
      _admitted.addAll(ids);
      _waiting.clear();
    }
    if (arrivals.isNotEmpty) {
      _unsaid.addAll(arrivals);
      _arrivals++;
    }
    _relayout();
    ref.read(pendingArrivalsProvider.notifier).report(<FlowId>{..._waiting});
    _scheduleAnnouncement();
  }

  /// Takes the waiting arrivals in, if nothing holds them back any more.
  void _mergeIfIdle() {
    if (!mounted || _frozen || _waiting.isEmpty) {
      return;
    }
    _merge();
  }

  /// Takes the waiting arrivals in now: the pill was clicked, or `Shift+J`.
  void _merge() {
    if (!mounted || _waiting.isEmpty) {
      return;
    }
    _admitted.addAll(_waiting);
    _waiting.clear();
    _relayout();
    ref.read(pendingArrivalsProvider.notifier).report(const <FlowId>{});
  }

  /// Rebuilds the lines and animates the difference.
  void _relayout() {
    final List<QueueItem> before = _items;
    final List<QueueItem> next = _lines();
    final List<QueueEdit> edits = listDiff(
      queueItemKeys(before),
      queueItemKeys(next),
    );
    setState(() => _items = next);
    final AnimatedListState? list = _listKey.currentState;
    if (list == null || edits.isEmpty) {
      return;
    }
    final List<QueueItem> working = List<QueueItem>.of(before);
    int arriving = 0;
    for (final QueueEdit edit in edits) {
      switch (edit.kind) {
        case QueueEditKind.remove:
          final QueueItem gone = working.removeAt(edit.index);
          // A line that folded away is still in the queue; only a line that
          // left the queue takes the exit of 2.4, with its direction.
          final bool folded = _folded(gone);
          list.removeItem(
            edit.index,
            (BuildContext context, Animation<double> animation) =>
                _LeavingLine(item: gone, animation: animation, folded: folded),
            duration: folded ? HMotion.arrive : HMotion.leave,
          );
        case QueueEditKind.insert:
          final QueueItem item = next[edit.index];
          working.insert(edit.index, item);
          final int step = math.min(arriving, HMotion.staggerMax - 1);
          final Duration wait = HMotion.stagger * step;
          final Duration total = HMotion.arrive + wait;
          _stagger[item.key] = wait.inMicroseconds / total.inMicroseconds;
          list.insertItem(edit.index, duration: total);
          arriving++;
      }
    }
  }

  /// True when [item] disappeared because its group folded, not because the
  /// request left the queue.
  bool _folded(QueueItem item) => switch (item) {
    QueueGroupHeader() => true,
    QueueFlowRow(:final Flow flow) => _admitted.contains(flow.id),
  };

  void _keyNavigated() {
    _keyNavRecent = true;
    _keyTimer?.cancel();
    _keyTimer = Timer(HMotion.freezeAfterKey, () {
      _keyNavRecent = false;
      _mergeIfIdle();
    });
  }

  void _pointerEnter() {
    _pointerTimer?.cancel();
    _pointerInside = true;
  }

  void _pointerLeave() {
    _pointerInside = false;
    _pointerTimer?.cancel();
    _pointerTimer = Timer(HMotion.freezeAfterPointer, _mergeIfIdle);
  }

  /// Says a burst of arrivals once, and then at most every
  /// [arrivalAnnounceWindow].
  void _scheduleAnnouncement() {
    if (_announceTimer != null || _unsaid.isEmpty) {
      return;
    }
    _sayArrivals();
    _announceTimer = Timer(arrivalAnnounceWindow, () {
      _announceTimer = null;
      _scheduleAnnouncement();
    });
  }

  void _sayArrivals() {
    if (!mounted || _unsaid.isEmpty) {
      return;
    }
    final Flow oldest = _unsaid.first;
    announcePolitely(
      context,
      context.l10n.interceptArrivals(
        _unsaid.length,
        oldest.methodLabel,
        oldest.host,
      ),
    );
    _unsaid.clear();
  }

  void _selectOnly(FlowId id) {
    ref.read(selectionProvider.notifier).clear();
    ref.read(selectedFlowIdProvider.notifier).select(id);
  }

  void _toggle(FlowId id) {
    // The toggle reads the cursor as its starting point, so it happens before
    // the cursor moves to the row that was clicked.
    ref.read(selectionProvider.notifier).toggle(id);
    ref.read(selectedFlowIdProvider.notifier).select(id);
  }

  void _range(FlowId id, HeldGroup group) {
    ref.read(selectionProvider.notifier).range(id, group.ids);
    ref.read(selectedFlowIdProvider.notifier).select(id);
  }

  void _selectGroup(HeldGroup group) {
    ref.read(selectionProvider.notifier).all(group.ids);
    ref.read(selectedFlowIdProvider.notifier).select(group.flows.first.id);
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(visibleQueueFlowsProvider, (
      QueueSnapshot? previous,
      QueueSnapshot next,
    ) {
      _sync(next.flows);
    });
    ref.listen(expandedGroupsProvider, (
      Map<String, bool>? previous,
      Map<String, bool> next,
    ) {
      _relayout();
    });
    ref.listen(queueKeyboardNavProvider, (int? previous, int next) {
      _keyNavigated();
    });
    ref.listen(queueMergeRequestProvider, (int? previous, int next) {
      // `Shift+J`: the key half of the pill (`docs/UX.md` 2.8, 5.1).
      _merge();
    });
    ref.listen(selectionProvider, (Set<FlowId>? previous, Set<FlowId> next) {
      if (next.length <= 1) {
        _mergeIfIdle();
      }
    });
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final FlowId? cursor = ref.watch(selectedFlowIdProvider);
    final Set<FlowId> members = ref.watch(selectionProvider);
    // A scalar, never `.length` on a watched collection (`docs/UX.md` 7).
    final int held = ref.watch(
      heldFlowsProvider.select((List<Flow> flows) => flows.length),
    );
    // A scalar, never the set itself (`docs/UX.md` 7).
    final int pending = ref.watch(
      pendingArrivalsProvider.select((Set<FlowId> waiting) => waiting.length),
    );
    return DecoratedBox(
      decoration: BoxDecoration(
        color: tokens.colors.bg1,
        border: Border(right: BorderSide(color: tokens.colors.line)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Padding(
            padding: EdgeInsets.fromLTRB(
              tokens.spacing.x3,
              tokens.spacing.x2,
              tokens.spacing.x3,
              tokens.spacing.x2,
            ),
            child: Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    l10n.interceptQueueTitle,
                    style: tokens.typography.ui13.semibold.tinted(
                      tokens.colors.fg0,
                    ),
                  ),
                ),
                QueueCountBadge(count: held, arrivals: _arrivals),
              ],
            ),
          ),
          const HHairline(),
          Expanded(
            child: MouseRegion(
              onEnter: (PointerEnterEvent _) => _pointerEnter(),
              onExit: (PointerExitEvent _) => _pointerLeave(),
              child: Stack(
                fit: StackFit.expand,
                children: <Widget>[
                  AnimatedList(
                    key: _listKey,
                    initialItemCount: _items.length,
                    padding: EdgeInsets.symmetric(vertical: tokens.spacing.x1),
                    itemBuilder:
                        (
                          BuildContext context,
                          int index,
                          Animation<double> animation,
                        ) {
                          if (index >= _items.length) {
                            return const SizedBox.shrink();
                          }
                          final QueueItem item = _items[index];
                          return _ArrivingLine(
                            animation: animation,
                            start: _stagger[item.key] ?? 0,
                            child: _line(item, cursor, members),
                          );
                        },
                  ),
                  if (_items.isEmpty) const QueueEmptyState(),
                  if (pending > 0)
                    NewArrivalsPill(count: pending, onMerge: _merge),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _line(QueueItem item, FlowId? cursor, Set<FlowId> members) =>
      switch (item) {
        QueueGroupHeader(:final HeldGroup group) => GroupHeaderRow(
          key: ValueKey<String>(item.key),
          group: group,
          open: ref.read(expandedGroupsProvider.notifier).isOpen(group),
          selected: members.isNotEmpty && group.ids.every(members.contains),
          onToggle: () =>
              ref.read(expandedGroupsProvider.notifier).toggle(group),
          onSelect: () => _selectGroup(group),
        ),
        QueueFlowRow(:final Flow flow, :final HeldGroup group) => QueueRow(
          key: ValueKey<String>(item.key),
          flow: flow,
          selected: flow.id == cursor,
          member: members.contains(flow.id),
          onSelect: () => _selectOnly(flow.id),
          onToggleSelect: () => _toggle(flow.id),
          onRangeSelect: () => _range(flow.id, group),
        ),
      };
}

/// The counter in the head of the queue.
///
/// It pulses once per burst of arrivals -- `held` at ten percent to twenty and
/// back, twice 120 ms -- so that something says "one came in" even when the
/// list is frozen. The digit inside jumps, it never animates
/// (`docs/UX.md` 2.2 and 2.9).
class QueueCountBadge extends StatefulWidget {
  /// Creates the badge for [count] held requests.
  const QueueCountBadge({
    required this.count,
    required this.arrivals,
    super.key,
  });

  /// How many requests wait.
  final int count;

  /// A number that grows with every burst; a change makes the badge pulse.
  final int arrivals;

  @override
  State<QueueCountBadge> createState() => _QueueCountBadgeState();
}

class _QueueCountBadgeState extends State<QueueCountBadge>
    with SingleTickerProviderStateMixin {
  late final AnimationController _pulse = AnimationController(
    vsync: this,
    duration: HMotion.press,
  );
  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _pulse,
    curve: Curves.easeInOut,
  );

  @override
  void didUpdateWidget(QueueCountBadge old) {
    super.didUpdateWidget(old);
    if (widget.arrivals != old.arrivals) {
      // There and back again: 2 × 120 ms.
      _pulse.forward(from: 0).whenComplete(() {
        if (mounted) {
          _pulse.reverse();
        }
      });
    }
  }

  @override
  void dispose() {
    _curve.dispose();
    _pulse.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Color held = tokens.state.held;
    final Widget badge = HBadge(
      text: l10n.interceptQueueCount(widget.count),
      color: widget.count > 0 ? held : tokens.colors.fg2,
    );
    return Stack(
      alignment: Alignment.center,
      children: <Widget>[
        Positioned.fill(
          child: Center(
            child: SizedBox(
              height: HBadge.chipHeight,
              child: FadeTransition(
                opacity: _curve,
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    color: held.withValues(alpha: HColors.tintAlpha),
                    borderRadius: HRadius.badgeRadius,
                  ),
                ),
              ),
            ),
          ),
        ),
        badge,
      ],
    );
  }
}

/// A line on its way in: eight pixels from above plus a fade.
///
/// The distance runs through [HReducedMotion]; the fade keeps its full
/// duration, because reduced motion means less way, not less answer
/// (`docs/UX.md` 2.10). [start] delays the line inside its own duration, so
/// that several arrivals in one frame come in one after the other.
class _ArrivingLine extends StatefulWidget {
  const _ArrivingLine({
    required this.animation,
    required this.start,
    required this.child,
  });

  final Animation<double> animation;
  final double start;
  final Widget child;

  @override
  State<_ArrivingLine> createState() => _ArrivingLineState();
}

class _ArrivingLineState extends State<_ArrivingLine> {
  // A field, never an expression in `build`: a `CurvedAnimation` that nobody
  // disposes is a listener that nobody removes (`docs/UX.md` 7).
  late final CurvedAnimation _curve = CurvedAnimation(
    parent: widget.animation,
    curve: Interval(widget.start, 1, curve: HMotion.enter),
  );

  @override
  void dispose() {
    _curve.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // A fraction of the row, not a pixel count: `SlideTransition` measures in
    // the size of its child, and the row grows with the text scale.
    final double slide =
        HReducedMotion.distance(context, HMotion.arriveOffset) / HSize.row;
    return FadeTransition(
      opacity: _curve,
      child: SlideTransition(
        position: _curve.drive(
          Tween<Offset>(begin: Offset(0, -slide), end: Offset.zero),
        ),
        child: RepaintBoundary(child: widget.child),
      ),
    );
  }
}

/// A line on its way out.
///
/// Two phases that overlap (`docs/UX.md` 2.4): glide and fade run in the first
/// sixty percent of [HMotion.leave], the collapse of the height in the last
/// sixty. Allowed glides to the right, blocked and timed out to the left.
///
/// Under reduced motion the line keeps its height and fades in place: a line
/// whose height falls to zero at once has nothing left to fade, and the fade
/// is the answer that must not be lost (`docs/UX.md` 2.10). A line that only
/// folded away takes neither direction: nothing left the queue.
class _LeavingLine extends StatefulWidget {
  const _LeavingLine({
    required this.item,
    required this.animation,
    required this.folded,
  });

  final QueueItem item;
  final Animation<double> animation;
  final bool folded;

  @override
  State<_LeavingLine> createState() => _LeavingLineState();
}

class _LeavingLineState extends State<_LeavingLine> {
  /// The animation of a removal runs from one to zero: the fade is through
  /// when it passes [HMotion.leaveGlideFraction], the height starts there.
  late final CurvedAnimation _fade = CurvedAnimation(
    parent: widget.animation,
    curve: Interval(1 - HMotion.leaveGlideFraction, 1, curve: HMotion.exit),
  );
  late final CurvedAnimation _size = CurvedAnimation(
    parent: widget.animation,
    curve: Interval(0, HMotion.leaveGlideFraction, curve: HMotion.exit),
  );
  late final CurvedAnimation _whole = CurvedAnimation(
    parent: widget.animation,
    curve: HMotion.exit,
  );

  @override
  void dispose() {
    _fade.dispose();
    _size.dispose();
    _whole.dispose();
    super.dispose();
  }

  double get _direction => switch (widget.item) {
    QueueFlowRow(:final Flow flow)
        when flow.decision == DecisionKind.allow ||
            flow.decision == DecisionKind.allowEdited =>
      1,
    _ => -1,
  };

  @override
  Widget build(BuildContext context) {
    final bool reduced = HReducedMotion.of(context);
    final double slide = widget.folded
        ? 0
        : HReducedMotion.distance(context, HMotion.leaveOffset) / HSize.row;
    final Animation<double> fade = reduced || widget.folded ? _whole : _fade;
    return SizeTransition(
      // Under reduced motion the line keeps its height and fades in place.
      sizeFactor: reduced ? const AlwaysStoppedAnimation<double>(1) : _size,
      child: FadeTransition(
        opacity: fade,
        child: SlideTransition(
          position: fade.drive(
            Tween<Offset>(
              begin: Offset(_direction * slide, 0),
              end: Offset.zero,
            ),
          ),
          child: RepaintBoundary(child: _FrozenLine(item: widget.item)),
        ),
      ),
    );
  }
}

/// A line that is leaving: a still image.
///
/// It observes nothing: its countdown stands from the moment of the decision,
/// and it keeps the height it had then. A line that keeps counting while it
/// leaves claims something that is not true (`docs/UX.md` 2.4).
class _FrozenLine extends StatelessWidget {
  const _FrozenLine({required this.item});

  final QueueItem item;

  @override
  Widget build(BuildContext context) => IgnorePointer(
    child: switch (item) {
      QueueFlowRow(:final Flow flow) => QueueRow(
        flow: flow,
        selected: false,
        onSelect: _nothing,
      ),
      QueueGroupHeader(:final HeldGroup group) => GroupHeaderRow(
        group: group,
        open: false,
        selected: false,
        onToggle: _nothing,
        onSelect: _nothing,
      ),
    },
  );

  static void _nothing() {}
}

/// What the pane shows while nothing waits: no spinner, one quiet sentence.
class QueueEmptyState extends StatelessWidget {
  /// Creates the empty state.
  const QueueEmptyState({super.key});

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Center(
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x6),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            ExcludeSemantics(
              child: CustomPaint(
                size: const Size.square(32),
                painter: _InboxPainter(color: tokens.colors.fg2),
              ),
            ),
            SizedBox(height: tokens.spacing.x4),
            Text(
              l10n.interceptEmptyTitle,
              textAlign: TextAlign.center,
              // Every sentence somebody is meant to read is `fg1` or better;
              // `fg2` is for controls that are really disabled
              // (`docs/UX.md` 6).
              style: tokens.typography.ui13.medium.tinted(tokens.colors.fg1),
            ),
            SizedBox(height: tokens.spacing.x2),
            Text(
              l10n.interceptEmptyHint,
              textAlign: TextAlign.center,
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
            SizedBox(height: tokens.spacing.x4),
            // The empty queue is the only teaching surface the program gets,
            // and it teaches the three keys once. The reversible key comes
            // first, and `Enter` names its consequence (`docs/UX.md` 4.1).
            Text(
              l10n.interceptEmptyKeys,
              textAlign: TextAlign.center,
              style: tokens.typography.mono12.tinted(tokens.colors.fg1),
            ),
          ],
        ),
      ),
    );
  }
}

/// Lucide `inbox`, painted like the glyphs of `packages/ui`.
class _InboxPainter extends CustomPainter {
  _InboxPainter({required this.color});

  final Color color;

  static const double _viewBox = 24;

  @override
  void paint(Canvas canvas, Size size) {
    final double scale = size.shortestSide / _viewBox;
    canvas.save();
    canvas.scale(scale, scale);
    final Paint stroke = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.6
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round
      ..color = color;
    final Path body = Path()
      ..moveTo(2, 12)
      ..lineTo(5.45, 5.11)
      ..lineTo(7.24, 4)
      ..lineTo(16.76, 4)
      ..lineTo(18.55, 5.11)
      ..lineTo(22, 12)
      ..lineTo(22, 18)
      ..lineTo(20, 20)
      ..lineTo(4, 20)
      ..lineTo(2, 18)
      ..close();
    final Path tray = Path()
      ..moveTo(22, 12)
      ..lineTo(16, 12)
      ..lineTo(14, 15)
      ..lineTo(10, 15)
      ..lineTo(8, 12)
      ..lineTo(2, 12);
    canvas
      ..drawPath(body, stroke)
      ..drawPath(tray, stroke)
      ..restore();
  }

  @override
  bool shouldRepaint(_InboxPainter oldDelegate) => oldDelegate.color != color;
}
