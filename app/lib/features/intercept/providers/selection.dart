/// The multi-selection of the queue (HUM-029).
///
/// Two different things, deliberately apart (`docs/UX.md` 3.5): the **cursor**
/// is [selectedFlowIdProvider] -- exactly one row, and the only one that
/// carries a fill -- while **membership** lives here and is drawn as the
/// accent rail over the full four pixels. An empty set means "the cursor
/// alone"; that keeps every single-request path of HUM-028 untouched.
library;

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import 'flows.dart';

part 'selection.g.dart';

/// Which held flows the next decision covers, beyond the cursor.
///
/// Empty is the normal case and means the cursor alone. A member that leaves
/// the queue -- decided, timed out -- drops out at once: a selection that
/// counts requests nobody can see any more would make the action bar promise
/// a reach it does not have.
@Riverpod(keepAlive: true)
class Selection extends _$Selection {
  @override
  Set<FlowId> build() {
    ref.listen(heldFlowsProvider, (List<Flow>? previous, List<Flow> next) {
      _prune(<FlowId>{for (final Flow flow in next) flow.id});
    });
    return const <FlowId>{};
  }

  void _prune(Set<FlowId> held) {
    if (state.isEmpty) {
      return;
    }
    final Set<FlowId> kept = state.where(held.contains).toSet();
    if (kept.length != state.length) {
      state = kept.length <= 1 ? const <FlowId>{} : kept;
    }
  }

  /// Drops the multi-selection; the cursor keeps its row.
  void clear() {
    if (state.isNotEmpty) {
      state = const <FlowId>{};
    }
  }

  /// Adds [id] to the selection or takes it out again (`Ctrl` and a click).
  ///
  /// The first toggle starts from the cursor, because that is what the person
  /// sees selected before they hold `Ctrl`.
  void toggle(FlowId id) {
    final FlowId? cursor = ref.read(selectedFlowIdProvider);
    final Set<FlowId> from = state.isEmpty ? <FlowId>{?cursor} : state;
    final Set<FlowId> next = <FlowId>{...from};
    if (!next.remove(id)) {
      next.add(id);
    }
    state = next.length <= 1 ? const <FlowId>{} : next;
  }

  /// Selects everything between the cursor and [id] inside [within]
  /// (`Shift` and a click).
  ///
  /// The range stays inside one group: a range across groups would reach
  /// further than the two rows the person pointed at.
  void range(FlowId id, List<FlowId> within) {
    final FlowId? cursor = ref.read(selectedFlowIdProvider);
    final int to = within.indexOf(id);
    final int from = cursor == null ? -1 : within.indexOf(cursor);
    if (to < 0) {
      return;
    }
    if (from < 0) {
      state = const <FlowId>{};
      return;
    }
    final int first = from < to ? from : to;
    final int last = from < to ? to : from;
    final Set<FlowId> next = within.sublist(first, last + 1).toSet();
    state = next.length <= 1 ? const <FlowId>{} : next;
  }

  /// Selects exactly [ids] (`Ctrl+A` inside a group, a click on a header).
  void all(Iterable<FlowId> ids) {
    final Set<FlowId> next = ids.toSet();
    state = next.length <= 1 ? const <FlowId>{} : next;
  }
}

/// The request a rule is generalised from.
///
/// The first of the reach, and the same one the notifier writes the rule from
/// (`InterceptDecision._many`): with a group of several hosts the cursor is
/// not necessarily that request, and a sentence that named the cursor while
/// the rule named the first row would guard an irreversible act with the wrong
/// host (`backlog/CONVENTIONS.md` 4.13).
@Riverpod(keepAlive: true)
Flow? ruleFlow(Ref ref) {
  final List<Flow> chosen = ref.watch(selectedFlowsProvider).flows;
  return chosen.isEmpty ? null : chosen.first;
}

/// What the next decision covers: the members, or the cursor alone.
///
/// A [QueueSnapshot] and not a bare list, so a rebuild of the queue that
/// changes nothing here changes nothing on the action bar either
/// (`docs/UX.md` 7).
@Riverpod(keepAlive: true)
QueueSnapshot selectedFlows(Ref ref) {
  final Set<FlowId> members = ref.watch(selectionProvider);
  if (members.isEmpty) {
    final Flow? cursor = ref.watch(selectedFlowProvider);
    return cursor == null ? QueueSnapshot.empty : QueueSnapshot(<Flow>[cursor]);
  }
  final List<Flow> held = ref.watch(heldFlowsProvider);
  return QueueSnapshot(<Flow>[
    for (final Flow flow in held)
      if (members.contains(flow.id)) flow,
  ]);
}
