/// The left pane: the queue of held requests.
///
/// The list is an `AnimatedList` fed from `visibleQueueFlowsProvider`, and the
/// difference between two snapshots is computed over flow ids, never over
/// positions (HUM-020 Fallstricke). Arrival slides in from above, a decided
/// row collapses and glides towards the side its decision points at.
library;

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../list_diff.dart';
import '../providers/flows.dart';
import 'queue_row.dart';

/// How far an arriving row slides down into place.
const double arriveOffset = HMotion.arriveOffset;

/// How far a decided row glides sideways while it leaves.
const double leaveOffset = 12;

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
  late List<Flow> _rows;

  @override
  void initState() {
    super.initState();
    _rows = ref.read(visibleQueueFlowsProvider).flows;
  }

  void _sync(QueueSnapshot next) {
    if (!mounted) {
      return;
    }
    final List<Flow> before = _rows;
    final List<QueueEdit> edits = listDiff(
      <FlowId>[for (final Flow flow in before) flow.id],
      <FlowId>[for (final Flow flow in next.flows) flow.id],
    );
    setState(() => _rows = next.flows);
    final AnimatedListState? list = _listKey.currentState;
    if (list == null) {
      return;
    }
    final List<Flow> working = List<Flow>.of(before);
    for (final QueueEdit edit in edits) {
      switch (edit.kind) {
        case QueueEditKind.remove:
          final Flow gone = working.removeAt(edit.index);
          list.removeItem(
            edit.index,
            (BuildContext context, Animation<double> animation) =>
                _LeavingRow(flow: gone, animation: animation),
            duration: HMotion.leave,
          );
        case QueueEditKind.insert:
          working.insert(edit.index, next.flows[edit.index]);
          list.insertItem(edit.index, duration: HMotion.arrive);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(visibleQueueFlowsProvider, (
      QueueSnapshot? previous,
      QueueSnapshot next,
    ) {
      _sync(next);
    });
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final FlowId? selected = ref.watch(selectedFlowIdProvider);
    final int held = ref.watch(heldFlowsProvider).length;
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
              tokens.spacing.x2,
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
                HBadge(
                  text: l10n.interceptQueueCount(held),
                  color: held > 0 ? tokens.state.held : tokens.colors.fg2,
                ),
              ],
            ),
          ),
          const HHairline(),
          Expanded(
            child: Stack(
              fit: StackFit.expand,
              children: <Widget>[
                AnimatedList(
                  key: _listKey,
                  initialItemCount: _rows.length,
                  padding: EdgeInsets.symmetric(vertical: tokens.spacing.x1),
                  itemBuilder:
                      (
                        BuildContext context,
                        int index,
                        Animation<double> animation,
                      ) {
                        if (index >= _rows.length) {
                          return const SizedBox.shrink();
                        }
                        final Flow flow = _rows[index];
                        return _ArrivingRow(
                          animation: animation,
                          child: QueueRow(
                            key: ValueKey<String>(flow.id.value),
                            flow: flow,
                            selected: flow.id == selected,
                            onSelect: () => ref
                                .read(selectedFlowIdProvider.notifier)
                                .select(flow.id),
                          ),
                        );
                      },
                ),
                if (_rows.isEmpty) const QueueEmptyState(),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// A row on its way in: eight pixels from above plus a fade.
class _ArrivingRow extends StatelessWidget {
  const _ArrivingRow({required this.animation, required this.child});

  final Animation<double> animation;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final Animation<double> curved = CurvedAnimation(
      parent: animation,
      curve: HMotion.enter,
    );
    return AnimatedBuilder(
      animation: curved,
      builder: (BuildContext context, Widget? child) => Opacity(
        opacity: curved.value,
        child: Transform.translate(
          offset: Offset(0, -arriveOffset * (1 - curved.value)),
          child: child,
        ),
      ),
      child: child,
    );
  }
}

/// A row on its way out: it collapses, fades, and glides towards the side its
/// decision points at -- allowed to the right, blocked and timed out to the
/// left.
class _LeavingRow extends StatelessWidget {
  const _LeavingRow({required this.flow, required this.animation});

  final Flow flow;
  final Animation<double> animation;

  double get _direction =>
      flow.decision == DecisionKind.allow ||
          flow.decision == DecisionKind.allowEdited
      ? 1
      : -1;

  @override
  Widget build(BuildContext context) {
    final Animation<double> curved = CurvedAnimation(
      parent: animation,
      curve: HMotion.exit,
    );
    return SizeTransition(
      sizeFactor: curved,
      child: AnimatedBuilder(
        animation: curved,
        builder: (BuildContext context, Widget? child) => Opacity(
          opacity: curved.value.clamp(0.0, 1.0),
          child: Transform.translate(
            offset: Offset(_direction * leaveOffset * (1 - curved.value), 0),
            child: child,
          ),
        ),
        child: QueueRow(flow: flow, selected: false, onSelect: () {}),
      ),
    );
  }
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
              style: tokens.typography.ui13.medium.tinted(tokens.colors.fg2),
            ),
            SizedBox(height: tokens.spacing.x2),
            Text(
              l10n.interceptEmptyHint,
              textAlign: TextAlign.center,
              style: tokens.typography.ui12.tinted(tokens.colors.fg2),
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
