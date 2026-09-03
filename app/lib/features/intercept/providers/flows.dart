/// The queue: the event stream, the flows it produces, the selection and the
/// decision that leaves again (CONVENTIONS 3.9, HUM-020).
///
/// State flows in one direction (ARCHITECTURE 5): `Subscribe` feeds
/// [flowEventsProvider], [Flows] folds the events into a map, everything else
/// is derived from that map. Widgets never call the client; they call the
/// notifiers here.

library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_diagnostics.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ipc/client_providers.dart';
import 'now.dart';

part 'flows.freezed.dart';
part 'flows.g.dart';

/// How long a decided flow stays in the queue so that the outcome can be seen
/// and the exit animation has something to animate.
const Duration queueExitWindow = Duration(seconds: 3);

/// The longest wait between two reconnect attempts.
const Duration maxReconnectBackoff = Duration(seconds: 30);

/// The filter that reloads the queue after a gap in the stream.
const FlowFilter heldFlowsFilter = FlowFilter(query: 'state:held');

/// The first wait after the event stream failed; it doubles up to
/// [maxReconnectBackoff]. Tests override it to keep themselves short.
@Riverpod(keepAlive: true)
Duration reconnectBackoff(Ref ref) => const Duration(seconds: 1);

/// `Subscribe`, kept alive across daemon restarts.
///
/// A broken stream is retried with 1 s, 2 s, 4 s ... up to
/// [maxReconnectBackoff]. Every reconnect starts with a synthetic
/// [FlowEvent.lagged], because everything that happened while the app was not
/// listening is exactly what `Lagged` means; [Flows] answers it with the same
/// `ListFlows` resync it uses for a real gap.
///
/// Written with an explicit subscription rather than as an `async*` generator
/// so that `ref.onDispose` can cancel the source at once: a generator is only
/// cancelled when it next resumes, which leaves the daemon -- or the fake --
/// holding a timer nobody waits for.
@Riverpod(keepAlive: true)
Stream<FlowEvent> flowEvents(Ref ref) {
  final DaemonClient client = ref.watch(daemonClientProvider);
  final Duration base = ref.watch(reconnectBackoffProvider);
  final StreamController<FlowEvent> events = StreamController<FlowEvent>();
  StreamSubscription<FlowEvent>? source;
  Timer? retry;
  Duration wait = base;
  bool disposed = false;

  void scheduleReconnect() {
    source = null;
    if (disposed || events.isClosed) {
      return;
    }
    retry?.cancel();
    retry = Timer(wait, () {
      retry = null;
      final Duration doubled = wait * 2;
      wait = doubled > maxReconnectBackoff ? maxReconnectBackoff : doubled;
      connectFlowEvents(
        client: client,
        events: events,
        afterGap: true,
        onEvent: () => wait = base,
        onBroken: scheduleReconnect,
        attach: (StreamSubscription<FlowEvent> subscription) =>
            source = subscription,
        isDisposed: () => disposed,
      );
    });
  }

  ref.onDispose(() {
    disposed = true;
    retry?.cancel();
    unawaited(source?.cancel());
    unawaited(events.close());
  });

  connectFlowEvents(
    client: client,
    events: events,
    afterGap: false,
    onEvent: () => wait = base,
    onBroken: scheduleReconnect,
    attach: (StreamSubscription<FlowEvent> subscription) =>
        source = subscription,
    isDisposed: () => disposed,
  );
  return events.stream;
}

/// Subscribes [client] and pipes its events into [events].
///
/// Split out of [flowEvents] so that the first attempt and every retry take
/// exactly the same path. [onBroken] runs when the stream fails, [onEvent]
/// when it delivers, and [attach] receives the live subscription so the
/// provider can cancel it.
void connectFlowEvents({
  required DaemonClient client,
  required StreamController<FlowEvent> events,
  required bool afterGap,
  required VoidCallback onEvent,
  required VoidCallback onBroken,
  required void Function(StreamSubscription<FlowEvent> subscription) attach,
  required bool Function() isDisposed,
}) {
  if (isDisposed() || events.isClosed) {
    return;
  }
  if (afterGap) {
    events.add(FlowEvent.lagged(at: DateTime.now(), dropped: 0));
  }
  try {
    attach(
      client.subscribe().listen(
        (FlowEvent event) {
          onEvent();
          if (!events.isClosed) {
            events.add(event);
          }
        },
        // Why the stream broke is the connection gate's business; the queue
        // only has to come back.
        onError: (Object error, StackTrace stack) => onBroken(),
        onDone: () {
          if (!isDisposed() && !events.isClosed) {
            events.close();
          }
        },
        cancelOnError: true,
      ),
    );
  } on Object {
    onBroken();
  }
}

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

  /// Selects [id]; a click, or the queue handing over.
  void select(FlowId id) => state = id;

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

/// What the action bar is doing.
@freezed
sealed class DecisionProgress with _$DecisionProgress {
  /// Nothing is in flight.
  const factory DecisionProgress.idle() = DecisionIdle;

  /// `Decide` is in flight for [flowId].
  const factory DecisionProgress.sending({
    required FlowId flowId,
    required DecisionKind kind,
  }) = DecisionSending;

  /// The daemon refused; the card under the bar says why.
  const factory DecisionProgress.failed({
    required FlowId flowId,
    required Diagnostic diagnostic,
  }) = DecisionFailed;

  const DecisionProgress._();

  /// True while a decision waits for the daemon.
  bool get isSending => this is DecisionSending;
}

/// Sends decisions and remembers what came back.
///
/// The only mutation the intercept screen performs. Errors become a
/// [Diagnostic] and are shown inline; nothing here opens a modal.
@Riverpod(keepAlive: true)
class InterceptDecision extends _$InterceptDecision {
  @override
  DecisionProgress build() => const DecisionProgress.idle();

  /// Decides [id]. Does nothing while another decision is in flight.
  Future<void> send(FlowId id, Decision decision) async {
    if (state.isSending) {
      return;
    }
    state = DecisionProgress.sending(flowId: id, kind: decision.kind);
    try {
      await ref.read(daemonClientProvider).decide(id, decision);
      if (ref.mounted) {
        state = const DecisionProgress.idle();
      }
    } on DaemonException catch (error) {
      if (ref.mounted) {
        state = DecisionProgress.failed(
          flowId: id,
          diagnostic: error.diagnostic,
        );
      }
    } on Object catch (error) {
      if (ref.mounted) {
        state = DecisionProgress.failed(
          flowId: id,
          diagnostic: ClientDiagnostics.daemonUnreachable(
            socketPath: '?',
            detail: error.toString(),
          ),
        );
      }
    }
  }

  /// Forgets a failure, so the next selection starts clean.
  void clear() {
    if (!state.isSending) {
      state = const DecisionProgress.idle();
    }
  }
}
