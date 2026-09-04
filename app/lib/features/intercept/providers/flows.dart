/// The queue: the event stream, the flows it produces and the selection
/// (CONVENTIONS 3.9, HUM-020).
///
/// State flows in one direction (ARCHITECTURE 5): `Subscribe` feeds
/// [flowEventsProvider], [Flows] folds the events into a map, everything else
/// is derived from that map. Widgets never call the client; they call the
/// notifiers here. The decision itself lives next door in `decision.dart`.
///
/// The stream itself is not here but in `core/ipc/flow_events.dart`: the
/// history and the tray are projections of the same subscription, and a
/// feature may not import another feature (ARCHITECTURE 5).

library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/flow_events.dart';
import 'now.dart';

part 'flows.g.dart';

/// How long a decided flow stays in the queue so that the outcome can be seen
/// and the exit animation has something to animate.
const Duration queueExitWindow = Duration(seconds: 3);

/// The filter that reloads the queue after a gap in the stream.
const FlowFilter heldFlowsFilter = FlowFilter(query: 'state:held');

/// Every flow the app has heard of, by id.
///
/// The map is the single source for the queue and, later, for the history:
/// two projections of one stream (BACKLOG.md 5).
@Riverpod(keepAlive: true)
class Flows extends _$Flows {
  @override
  Map<FlowId, Flow> build() {
    // `fireImmediately` is what starts the stream: a listener alone does not
    // build the provider it listens to, and a queue that never subscribes
    // stays empty for good.
    ref.listen(flowEventsProvider, (
      AsyncValue<FlowEvent>? previous,
      AsyncValue<FlowEvent> next,
    ) {
      next.whenData(_apply);
    }, fireImmediately: true);
    return const <FlowId, Flow>{};
  }

  void _apply(FlowEvent event) {
    switch (event) {
      case FlowEventReceived(:final Flow flow):
        state = <FlowId, Flow>{...state, flow.id: flow};
      case FlowEventAnalyzed(
        :final FlowId flowId,
        :final List<Finding> findings,
      ):
        _update(
          flowId,
          (Flow flow) => flow.copyWith(
            state: flow.state == FlowState.received
                ? FlowState.analyzed
                : flow.state,
            findingCount: findings.length,
          ),
        );
      case FlowEventHeld(:final FlowId flowId, :final DateTime deadline):
        _update(
          flowId,
          (Flow flow) => flow.copyWith(
            state: FlowState.held,
            deadline: deadline,
            heldAt: event.at,
          ),
        );
      case FlowEventDecided(
        :final FlowId flowId,
        :final DecisionKind kind,
        :final DecisionSource? source,
        :final BlockReason? blockReason,
        :final RuleId? ruleId,
      ):
        _update(
          flowId,
          (Flow flow) => flow.copyWith(
            state: FlowState.decided,
            decision: kind,
            decisionSource: source,
            blockReason: blockReason,
            ruleId: ruleId,
            edited: kind == DecisionKind.allowEdited,
            decidedAt: event.at,
          ),
        );
      case FlowEventForwarded(:final FlowId flowId):
        _update(
          flowId,
          (Flow flow) => flow.copyWith(state: FlowState.forwarded),
        );
      case FlowEventResponseHeaders(
        :final FlowId flowId,
        :final HttpResponseHead head,
      ):
        _update(
          flowId,
          (Flow flow) =>
              flow.copyWith(state: FlowState.responded, status: head.status),
        );
      case FlowEventRecorded(:final FlowId flowId):
        _update(
          flowId,
          (Flow flow) => flow.copyWith(state: FlowState.recorded),
        );
      case FlowEventTimedOut(:final FlowId flowId):
        _update(
          flowId,
          (Flow flow) => flow.copyWith(
            state: FlowState.decided,
            decision: DecisionKind.timedOut,
            decisionSource: DecisionSource.timeout,
            blockReason: BlockReason.timeout,
            decidedAt: event.at,
          ),
        );
      case FlowEventFailed(:final FlowId flowId, :final UpstreamError error):
        _update(
          flowId,
          (Flow flow) =>
              flow.copyWith(state: FlowState.failed, upstreamError: error),
        );
      case FlowEventLagged():
        unawaited(_resync());
      // A chunk counter, a session diagnostic, a rule revision and an agent
      // question change no flow. The first is deliberate (v1 shows no
      // progress bar), the other three belong to other screens.
      case FlowEventResponseChunk() ||
          FlowEventDiagnostic() ||
          FlowEventRulesChanged() ||
          FlowEventAgentAsk():
        break;
    }
  }

  void _update(FlowId id, Flow Function(Flow flow) update) {
    final Flow? current = state[id];
    if (current == null) {
      // An event for a flow that arrived before this client did. The resync
      // after a gap brings the whole queue, so nothing is invented here.
      return;
    }
    state = <FlowId, Flow>{...state, id: update(current)};
  }

  /// Reloads the held flows after a gap in the stream.
  ///
  /// The answer is authoritative for what is held: flows this client still
  /// believes are held but the daemon no longer lists have left the queue
  /// while nobody was listening. What the client knows and the wire does not
  /// carry ([Flow.heldAt], [Flow.decidedAt]) is kept.
  Future<void> _resync() async {
    try {
      final FlowPage page = await ref
          .read(daemonClientProvider)
          .listFlows(heldFlowsFilter);
      if (!ref.mounted) {
        return;
      }
      final Map<FlowId, Flow> next = <FlowId, Flow>{
        for (final MapEntry<FlowId, Flow> entry in state.entries)
          if (!entry.value.isHeld) entry.key: entry.value,
      };
      for (final Flow flow in page.flows) {
        final Flow? known = state[flow.id];
        next[flow.id] = flow.copyWith(
          heldAt: flow.heldAt ?? known?.heldAt,
          decidedAt: flow.decidedAt ?? known?.decidedAt,
        );
      }
      state = next;
    } on Object {
      // The queue keeps what it has; the next event or the next reconnect
      // tries again.
    }
  }
}

/// The held flows, earliest deadline first (CONVENTIONS 3.9).
@Riverpod(keepAlive: true)
List<Flow> heldFlows(Ref ref) {
  final Map<FlowId, Flow> flows = ref.watch(flowsProvider);
  return flows.values.where((Flow flow) => flow.isHeld).toList()
    ..sort(compareByDeadline);
}

/// The rows the queue pane draws.
///
/// A separate type instead of a bare list because the snapshot is recomputed
/// with every tick of [nowProvider]: with value equality riverpod notices that
/// nothing changed and no row is rebuilt for a clock that moved.
@immutable
class QueueSnapshot {
  /// Wraps [flows], in queue order.
  const QueueSnapshot(this.flows);

  /// An empty queue.
  static const QueueSnapshot empty = QueueSnapshot(<Flow>[]);

  /// The rows, earliest deadline first.
  final List<Flow> flows;

  @override
  bool operator ==(Object other) =>
      other is QueueSnapshot && listEquals(flows, other.flows);

  @override
  int get hashCode => Object.hashAll(flows);
}

/// Held flows plus the ones decided within the last [queueExitWindow], so a
/// decision can be seen before its row collapses.
@Riverpod(keepAlive: true)
QueueSnapshot visibleQueueFlows(Ref ref) {
  final DateTime now = ref.watch(nowProvider);
  final Map<FlowId, Flow> flows = ref.watch(flowsProvider);
  final List<Flow> visible =
      flows.values
          .where((Flow flow) => flow.isHeld || leavingAt(flow, now))
          .toList()
        ..sort(compareByDeadline);
  return QueueSnapshot(visible);
}

/// True while [flow] is decided but still shown, within [queueExitWindow] of
/// [Flow.decidedAt].
bool leavingAt(Flow flow, DateTime now) {
  final DateTime? decidedAt = flow.decidedAt;
  if (decidedAt == null) {
    return false;
  }
  // A decision the clock has not caught up with yet -- the daemon stamps the
  // event, the app reads its own clock a moment earlier -- counts as just
  // taken, not as long gone.
  return now.difference(decidedAt) < queueExitWindow;
}

/// Queue order: deadline first, then arrival, then id.
///
/// A flow without a deadline sorts last; it is either not held yet or was
/// decided by a rule before anyone could wait for it.
int compareByDeadline(Flow a, Flow b) {
  final DateTime? left = a.deadline;
  final DateTime? right = b.deadline;
  if (left != null && right != null && left != right) {
    return left.compareTo(right);
  }
  if (left == null && right != null) {
    return 1;
  }
  if (left != null && right == null) {
    return -1;
  }
  final int arrival = a.receivedAt.compareTo(b.receivedAt);
  return arrival != 0 ? arrival : a.id.compareTo(b.id);
}

/// Which flow the card shows (CONVENTIONS 3.9).
///
/// Three rules, in this order: a new flow never steals the selection; an empty
/// selection takes the first flow that arrives; a selected flow that leaves
/// hands the selection to the next one in deadline order, never to the newest.
///
/// "Leaves" means two different moments. A decision the person took moves the
/// selection at once, so that a queue can be worked through with Enter alone.
/// A decision that happened to them -- a timeout, a rule -- keeps the card
/// until the row leaves the visible queue, because that card is the only place
/// the outcome is explained.
@Riverpod(keepAlive: true)
class SelectedFlowId extends _$SelectedFlowId {
  @override
  FlowId? build() {
    // The listener may not fire during this build: it reads `state`, and a
    // notifier that is still building has none. The first selection is
    // therefore taken here, and `ref.read` is also what starts the queue --
    // a listener alone does not build what it listens to.
    ref.listen(visibleQueueFlowsProvider, _follow);
    final List<Flow> queue = ref.read(visibleQueueFlowsProvider).flows;
    return queue.isEmpty ? null : queue.first.id;
  }

  /// True while a decision key is still down; the selection waits for it.
  bool _keyDown = false;

  /// True when the queue wanted to hand the selection on while [_keyDown].
  bool _deferred = false;

  /// Selects [id]; a click, or the queue handing over.
  void select(FlowId id) => state = id;

  /// Tells the selection that a decision key is down, or was released.
  ///
  /// While a key is down the selection stays where it is, even after a
  /// decision: the arming of `docs/UX.md` 5.4 measures how long the URL of a
  /// new selection has been readable, and a selection that moves under a
  /// finger which never came up would arm against nobody. On release the
  /// deferred hand-over happens at once.
  void setDecisionKeyDown(bool down) {
    if (_keyDown == down) {
      return;
    }
    _keyDown = down;
    if (!down && _deferred) {
      _deferred = false;
      _advance();
    }
  }

  /// Hands the selection to the next held flow, from wherever it stands now.
  void _advance() {
    final List<Flow> flows = ref.read(visibleQueueFlowsProvider).flows;
    if (flows.isEmpty) {
      state = null;
      return;
    }
    final int index = flows.indexWhere((Flow flow) => flow.id == state);
    if (index < 0) {
      state = flows.first.id;
      return;
    }
    final Flow? successor = _nextHeld(flows, index);
    if (successor != null) {
      state = successor.id;
    }
  }

  /// Selects the next row of the queue.
  void next() => _step(1);

  /// Selects the previous row of the queue.
  void previous() => _step(-1);

  void _step(int delta) {
    final List<Flow> flows = ref.read(visibleQueueFlowsProvider).flows;
    if (flows.isEmpty) {
      return;
    }
    final int current = flows.indexWhere((Flow flow) => flow.id == state);
    final int next = current < 0
        ? (delta > 0 ? 0 : flows.length - 1)
        : (current + delta).clamp(0, flows.length - 1);
    state = flows[next].id;
  }

  void _follow(QueueSnapshot? previous, QueueSnapshot next) {
    final List<Flow> flows = next.flows;
    if (flows.isEmpty) {
      state = null;
      return;
    }
    final FlowId? current = state;
    if (current == null) {
      state = flows.first.id;
      return;
    }
    final int index = flows.indexWhere((Flow flow) => flow.id == current);
    if (index >= 0) {
      final Flow selected = flows[index];
      if (selected.isHeld || selected.decisionSource != DecisionSource.user) {
        return;
      }
      if (_keyDown) {
        _deferred = true;
        return;
      }
      final Flow? successor = _nextHeld(flows, index);
      if (successor != null) {
        state = successor.id;
      }
      return;
    }
    final int before =
        previous?.flows.indexWhere((Flow flow) => flow.id == current) ?? 0;
    state = flows[before.clamp(0, flows.length - 1)].id;
  }

  Flow? _nextHeld(List<Flow> flows, int from) {
    for (int i = from + 1; i < flows.length; i++) {
      if (flows[i].isHeld) {
        return flows[i];
      }
    }
    for (int i = from - 1; i >= 0; i--) {
      if (flows[i].isHeld) {
        return flows[i];
      }
    }
    return null;
  }
}

/// The selected flow itself, or null.
@Riverpod(keepAlive: true)
Flow? selectedFlow(Ref ref) {
  final FlowId? id = ref.watch(selectedFlowIdProvider);
  if (id == null) {
    return null;
  }
  // The map is watched whole: riverpod compares the result of this provider
  // with `==`, and `Flow` is a value type, so a change to another flow ends
  // here instead of rebuilding the card.
  return ref.watch(flowsProvider)[id];
}

/// Everything the card shows beyond the row: headers, query, body preview.
@riverpod
Future<FlowDetail> flowDetail(Ref ref, FlowId id) =>
    ref.watch(daemonClientProvider).getFlow(id);
