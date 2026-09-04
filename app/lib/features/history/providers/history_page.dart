/// One answer to the current history query: the loaded rows, where the next
/// page begins, how many rows the filter matches, and what went wrong.
///
/// Paging is keyset, never offset: the cursor belongs to the *bottom* of the
/// list, so a flow that arrives at the top never shifts it and no page is
/// duplicated or skipped (`backlog/sprint-2.md`, HUM-032, Fallstricke, and
/// `backlog/CONVENTIONS.md` 4.14). Rows are matched by [FlowId], never by
/// position.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ipc/flow_events.dart';
import 'history_query.dart';

/// How many rows one page asks for. The wire default; the daemon caps at
/// 1000.
const int historyPageSize = 200;

/// How many rows the screen keeps in memory at once.
///
/// The history is unbounded, the window over it is not: two thousand rows are
/// more than a person reads in one sitting, and the filter is the way to the
/// rest (`backlog/sprint-2.md`, HUM-032). When the window is full the footer
/// says so rather than loading on silently.
const int historyMaxRows = 2000;

/// The rows of the current query, plus everything the screen has to say
/// honestly about them.
@immutable
class HistoryPageState {
  /// Creates a page state.
  const HistoryPageState({
    this.rows = const <Flow>[],
    this.pending = const <Flow>[],
    this.missed = 0,
    this.cursor = '',
    this.total = 0,
    this.capped = false,
    this.unfilteredTotal = -1,
    this.unfilteredCapped = false,
    this.hiddenPassthrough = false,
    this.loading = false,
    this.loadingMore = false,
    this.failure,
  });

  /// Nothing loaded yet.
  static const HistoryPageState empty = HistoryPageState(loading: true);

  /// The rows in the order the daemon returned them.
  final List<Flow> rows;

  /// Flows that arrived while the person was reading further down; they join
  /// [rows] when the list is back at its head or the pill is used.
  final List<Flow> pending;

  /// How many arrivals this query could not place itself.
  ///
  /// Under a filter only the recorder knows whether an arrival matches, so
  /// it is counted and the pill offers a reload instead of a merge.
  final int missed;

  /// How many arrivals the pill announces, however it will show them.
  int get waiting => pending.length + missed;

  /// Where the next page begins; empty when there is none.
  final String cursor;

  /// How many rows the filter matches, as the daemon counted them.
  final int total;

  /// True when [total] is only a lower bound, as the daemon said.
  ///
  /// Read from `FlowPage.capped`, never inferred from the value: a number
  /// that pretends to be exact is what `backlog/CONVENTIONS.md` 4.13 forbids,
  /// and the ceiling is the recorder's business, not the surface's.
  final bool capped;

  /// How many rows there are without any filter, or -1 while unknown.
  ///
  /// Only fetched when a filter matches nothing, because that is the one
  /// sentence that needs it (`docs/UX.md` 4.1).
  final int unfilteredTotal;

  /// True when [unfilteredTotal] is itself only a lower bound.
  final bool unfilteredCapped;

  /// True when the list is empty only because passthrough traffic is hidden.
  ///
  /// The ordinary case, not an exotic one: an agent calls its model first,
  /// so a young session can consist of nothing else. Saying "the record is
  /// open" over three hidden requests would be a claim about an emptiness
  /// that is none (`backlog/CONVENTIONS.md` 4.13).
  final bool hiddenPassthrough;

  /// True while the first page of a query is on its way.
  final bool loading;

  /// True while a further page is on its way.
  final bool loadingMore;

  /// Why the last request failed, or null.
  final Diagnostic? failure;

  /// True when another page can be fetched.
  bool get hasMore => cursor.isNotEmpty && rows.length < historyMaxRows;

  /// True when the window is full and the rest is only reachable by
  /// narrowing the filter.
  bool get windowFull => rows.length >= historyMaxRows && cursor.isNotEmpty;

  /// True when the query returned nothing at all.
  bool get isEmpty => rows.isEmpty && !loading;

  /// A copy with the named fields replaced. [failure] is cleared by passing
  /// [clearFailure], because null means "unchanged" everywhere else.
  HistoryPageState copyWith({
    List<Flow>? rows,
    List<Flow>? pending,
    int? missed,
    String? cursor,
    int? total,
    bool? capped,
    int? unfilteredTotal,
    bool? unfilteredCapped,
    bool? hiddenPassthrough,
    bool? loading,
    bool? loadingMore,
    Diagnostic? failure,
    bool clearFailure = false,
  }) => HistoryPageState(
    rows: rows ?? this.rows,
    pending: pending ?? this.pending,
    missed: missed ?? this.missed,
    cursor: cursor ?? this.cursor,
    total: total ?? this.total,
    capped: capped ?? this.capped,
    unfilteredTotal: unfilteredTotal ?? this.unfilteredTotal,
    unfilteredCapped: unfilteredCapped ?? this.unfilteredCapped,
    hiddenPassthrough: hiddenPassthrough ?? this.hiddenPassthrough,
    loading: loading ?? this.loading,
    loadingMore: loadingMore ?? this.loadingMore,
    failure: clearFailure ? null : (failure ?? this.failure),
  );

  @override
  bool operator ==(Object other) =>
      other is HistoryPageState &&
      listEquals(other.rows, rows) &&
      listEquals(other.pending, pending) &&
      other.missed == missed &&
      other.cursor == cursor &&
      other.total == total &&
      other.capped == capped &&
      other.unfilteredTotal == unfilteredTotal &&
      other.unfilteredCapped == unfilteredCapped &&
      other.hiddenPassthrough == hiddenPassthrough &&
      other.loading == loading &&
      other.loadingMore == loadingMore &&
      other.failure == failure;

  @override
  int get hashCode => Object.hash(
    Object.hashAll(rows),
    Object.hashAll(pending),
    missed,
    cursor,
    total,
    capped,
    unfilteredTotal,
    unfilteredCapped,
    hiddenPassthrough,
    loading,
    loadingMore,
    failure,
  );
}

/// The loaded page of the history.
///
/// Hand-written for the same reason as [historyQueryProvider]: the canonical
/// name is `historyPageProvider` and the state type is `HistoryPageState`.
final NotifierProvider<HistoryPageNotifier, HistoryPageState>
historyPageProvider = NotifierProvider<HistoryPageNotifier, HistoryPageState>(
  HistoryPageNotifier.new,
);

/// The notifier behind [historyPageProvider].
class HistoryPageNotifier extends Notifier<HistoryPageState> {
  /// Which load is the current one. An answer from an older query is dropped
  /// instead of being merged into a list it does not belong to.
  int _generation = 0;

  /// Whether the list is standing at its head.
  ///
  /// The table says so; the provider decides what follows from it. A flow
  /// that arrives while the head is on screen joins the rows at once, which
  /// is what `backlog/sprint-2.md` asks for; one that arrives while somebody
  /// reads further down waits in [HistoryPageState.pending], because nothing
  /// moves under the reading eye (`docs/UX.md` 2.8).
  bool _atHead = true;

  /// False while `build` is still running.
  ///
  /// The event listener fires immediately, and a notifier that is still
  /// building has no state to fold an event into. The first load covers
  /// whatever arrives in that instant anyway.
  bool _ready = false;

  @override
  HistoryPageState build() {
    ref.listen(historyQueryProvider, (HistoryQuery? previous, HistoryQuery _) {
      unawaited(reload());
    });
    // The event stream is the same subscription the queue folds; the history
    // only projects it differently (BACKLOG.md 5). Riverpod pauses the
    // listeners of a provider nobody listens to, so this fires while the
    // screen watches the page and rests while it does not -- which is
    // exactly the behaviour `docs/UX.md` 7 asks of a clock in an unseen
    // section.
    ref.listen(flowEventsProvider, (
      AsyncValue<FlowEvent>? previous,
      AsyncValue<FlowEvent> next,
    ) {
      next.whenData(_apply);
    }, fireImmediately: true);
    // Scheduled, not called: `reload` writes `state`, and a notifier that is
    // still building has none.
    unawaited(Future<void>.microtask(reload));
    _ready = true;
    return HistoryPageState.empty;
  }

  /// Throws away the loaded rows and asks for the first page of the current
  /// query.
  ///
  /// [keepSelection] leaves the rows on screen while the answer is on its
  /// way, so that a resync after a gap does not blank the table under the
  /// person reading it; the new page replaces them in one frame.
  Future<void> reload({bool keepSelection = false}) async {
    final int generation = ++_generation;
    state = state.copyWith(
      rows: keepSelection ? state.rows : const <Flow>[],
      pending: const <Flow>[],
      missed: 0,
      cursor: '',
      // Loading either way; the table only draws the skeleton when it has
      // no rows to keep, so a resync does not blank the list under the
      // person reading it (`docs/UX.md` 2.11).
      loading: true,
      loadingMore: false,
      clearFailure: true,
    );
    await _fetch(generation: generation, cursor: null);
  }

  /// Asks for the next page. Does nothing while one is on its way, when the
  /// daemon named no cursor, or when the window is full.
  Future<void> loadMore() async {
    final HistoryPageState current = state;
    if (current.loading || current.loadingMore || !current.hasMore) {
      return;
    }
    final int generation = _generation;
    state = current.copyWith(loadingMore: true);
    await _fetch(generation: generation, cursor: current.cursor);
  }

  /// Tells the provider whether the list stands at its head.
  ///
  /// Turning true flushes whatever waited: the reason to hold arrivals back
  /// is gone the moment the head is in view again.
  void setAtHead(bool atHead) {
    if (_atHead == atHead) {
      return;
    }
    _atHead = atHead;
    if (!atHead) {
      return;
    }
    // What could be placed joins the rows; what could not needs the daemon,
    // so the pill stays until somebody uses it.
    merge();
  }

  /// Fetches the page again, because arrivals could not be placed in it.
  Future<void> refresh() => reload(keepSelection: true);

  /// Moves the flows that arrived meanwhile to the top of the list.
  void merge() {
    final HistoryPageState current = state;
    if (current.pending.isEmpty) {
      return;
    }
    final List<Flow> rows = <Flow>[
      ...current.pending.reversed,
      ...current.rows,
    ];
    state = current.copyWith(
      rows: rows.length > historyMaxRows
          ? rows.sublist(0, historyMaxRows)
          : rows,
      pending: const <Flow>[],
      total: current.capped
          ? current.total
          : current.total + current.pending.length,
    );
  }

  Future<void> _fetch({
    required int generation,
    required String? cursor,
  }) async {
    final HistoryQuery query = ref.read(historyQueryProvider);
    final DaemonClient client = ref.read(daemonClientProvider);
    try {
      final FlowPage page = await client.listFlows(
        query.flowFilter,
        limit: historyPageSize,
        cursor: cursor,
      );
      if (!ref.mounted || generation != _generation) {
        return;
      }
      final List<Flow> rows = cursor == null
          ? page.flows
          : _append(state.rows, page.flows);
      final _EmptyProbe probe = await _probeEmpty(
        query: query,
        page: page,
        rowCount: rows.length,
      );
      if (!ref.mounted || generation != _generation) {
        return;
      }
      state = state.copyWith(
        rows: rows.length > historyMaxRows
            ? rows.sublist(0, historyMaxRows)
            : rows,
        cursor: page.nextCursor,
        total: page.total,
        capped: page.capped,
        unfilteredTotal: probe.total,
        unfilteredCapped: probe.capped,
        hiddenPassthrough: probe.hiddenPassthrough,
        loading: false,
        loadingMore: false,
        clearFailure: true,
      );
    } on DaemonException catch (error) {
      if (!ref.mounted || generation != _generation) {
        return;
      }
      state = state.copyWith(
        loading: false,
        loadingMore: false,
        failure: error.diagnostic,
      );
    }
  }

  /// How many rows there would be without the filter.
  ///
  /// Asked only when a filter matched nothing, because that is the only
  /// sentence that names it: "`host:foo` matches 0 of 1,284 requests"
  /// (`docs/UX.md` 4.1). One row is enough; the count comes with the page.
  /// What an empty answer means.
  ///
  /// Asked only when nothing came back, because that is the only moment the
  /// distinction matters: a filter that cuts everything away needs the total
  /// it cut from, and a list that is empty only because passthrough traffic
  /// is hidden needs to say so instead of claiming the record is empty.
  Future<_EmptyProbe> _probeEmpty({
    required HistoryQuery query,
    required FlowPage page,
    required int rowCount,
  }) async {
    if (rowCount > 0 || page.total > 0) {
      return query.isUnfiltered
          ? _EmptyProbe(total: page.total, capped: page.capped)
          : _EmptyProbe(
              total: state.unfilteredTotal,
              capped: state.unfilteredCapped,
            );
    }
    try {
      if (query.isUnfiltered) {
        if (query.includePassthrough) {
          return _EmptyProbe(total: page.total, capped: page.capped);
        }
        // Nothing without passthrough: is there anything with it?
        final FlowPage withLlm = await ref
            .read(daemonClientProvider)
            .listFlows(
              FlowFilter(orderBy: query.orderBy, includePassthrough: true),
              limit: 1,
            );
        return _EmptyProbe(
          total: withLlm.total,
          capped: withLlm.capped,
          hiddenPassthrough: withLlm.total > 0,
        );
      }
      final FlowPage all = await ref
          .read(daemonClientProvider)
          .listFlows(
            FlowFilter(
              orderBy: query.orderBy,
              includePassthrough: query.includePassthrough,
            ),
            limit: 1,
          );
      return _EmptyProbe(total: all.total, capped: all.capped);
    } on DaemonException {
      // The empty state then names the filter and the way back without a
      // number, which is better than naming a number nobody counted.
      return const _EmptyProbe(total: -1, capped: false);
    }
  }

  /// Appends [next] to [rows] without letting one flow appear twice.
  ///
  /// Keyset paging does not repeat a row, but a flow that arrived at the top
  /// and was merged can also come back in a later page; the id decides.
  List<Flow> _append(List<Flow> rows, List<Flow> next) {
    final Set<String> known = <String>{
      for (final Flow flow in rows) flow.id.value,
      // The buffer counts as known: a flow that waits there and arrives in
      // a later page would otherwise stand twice after the merge. Out of
      // reach while sorting by time, not while sorting by anything else.
      for (final Flow flow in state.pending) flow.id.value,
    };
    return <Flow>[
      ...rows,
      for (final Flow flow in next)
        if (known.add(flow.id.value)) flow,
    ];
  }

  /// Folds one event into the loaded rows, for a test that has no daemon.
  ///
  /// The stream is the only caller in the product; a test that wants to see
  /// one event land should not have to script a whole session for it.
  @visibleForTesting
  void applyEventForTest(FlowEvent event) => _apply(event);

  /// Folds one event into the loaded rows.
  ///
  /// A state change reaches the row it belongs to and moves nothing: the row
  /// keeps its place, so nothing shifts under the reading eye (`docs/UX.md`
  /// 2.8). An arrival is only taken when the query can place it without
  /// asking the daemon; otherwise only the recorder knows whether it matches.
  void _apply(FlowEvent event) {
    if (!_ready) {
      return;
    }
    switch (event) {
      case FlowEventReceived(:final Flow flow):
        _arrived(flow);
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
            deadline: null,
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
      case FlowEventResponseChunk(:final FlowId flowId, :final int bytesSoFar):
        // The counter is cumulative, so the row shows what has arrived so
        // far instead of adding a chunk to itself twice after a resync.
        _update(flowId, (Flow flow) => flow.copyWith(responseSize: bytesSoFar));
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
            deadline: null,
          ),
        );
      case FlowEventFailed(:final FlowId flowId, :final UpstreamError error):
        _update(
          flowId,
          (Flow flow) =>
              flow.copyWith(state: FlowState.failed, upstreamError: error),
        );
      case FlowEventLagged():
        // Everything that happened while nobody listened is exactly what
        // `Lagged` means, and the event stream raises one on every
        // reconnect. Without a reload a held row keeps offering "Open in
        // Intercept" for a flow the queue let go minutes ago, and a request
        // recorded during the gap is missing while the footer counts as if
        // it were not (`backlog/CONVENTIONS.md` 4.13).
        unawaited(reload(keepSelection: true));
      // A session diagnostic, a rule revision and an agent question change
      // no recorded row.
      case FlowEventDiagnostic() ||
          FlowEventRulesChanged() ||
          FlowEventAgentAsk():
        break;
    }
  }

  void _arrived(Flow flow) {
    final HistoryQuery query = ref.read(historyQueryProvider);
    if (flow.passthrough && !query.includePassthrough) {
      return;
    }
    if (!query.takesArrivals) {
      // Only the recorder knows whether this one matches the filter and
      // where it would sit. It is counted rather than dropped, and the pill
      // offers the reload that answers the question
      // (`backlog/sprint-2.md`: Pille „12 new · refresh").
      state = state.copyWith(missed: state.missed + 1);
      return;
    }
    final HistoryPageState current = state;
    final bool known =
        current.rows.any((Flow row) => row.id == flow.id) ||
        current.pending.any((Flow row) => row.id == flow.id);
    if (known) {
      return;
    }
    if (!_atHead) {
      state = current.copyWith(pending: <Flow>[...current.pending, flow]);
      return;
    }
    // The head is on screen, so the newest row belongs at the top now. The
    // cursor is not touched: it belongs to the bottom of the list, and a row
    // inserted above it moves no page boundary (`backlog/sprint-2.md`,
    // HUM-032, Fallstricke).
    final List<Flow> rows = <Flow>[flow, ...current.rows];
    state = current.copyWith(
      rows: rows.length > historyMaxRows
          ? rows.sublist(0, historyMaxRows)
          : rows,
      total: current.capped ? current.total : current.total + 1,
    );
  }

  void _update(FlowId id, Flow Function(Flow flow) update) {
    final HistoryPageState current = state;
    final int index = current.rows.indexWhere((Flow row) => row.id == id);
    if (index >= 0) {
      final List<Flow> rows = List<Flow>.of(current.rows);
      rows[index] = update(rows[index]);
      state = current.copyWith(rows: rows);
      return;
    }
    final int waiting = current.pending.indexWhere((Flow row) => row.id == id);
    if (waiting >= 0) {
      final List<Flow> pending = List<Flow>.of(current.pending);
      pending[waiting] = update(pending[waiting]);
      state = current.copyWith(pending: pending);
    }
  }
}

/// What an empty answer turned out to mean.
@immutable
class _EmptyProbe {
  const _EmptyProbe({
    required this.total,
    required this.capped,
    this.hiddenPassthrough = false,
  });

  /// How many rows there are without the text filter, or -1 while unknown.
  final int total;

  /// Whether [total] is itself only a lower bound.
  final bool capped;

  /// Whether the emptiness is only the hidden passthrough traffic.
  final bool hiddenPassthrough;
}
