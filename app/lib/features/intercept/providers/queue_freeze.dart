/// What the queue needs to keep still under a reading eye (HUM-029).
///
/// The frozen order itself is **not** here. It is view state and lives in the
/// `State` of the queue pane, where it dies with the pane and where pointer
/// movement never reaches the provider graph (`docs/UX.md` 7 and 8). Only two
/// things are providers, because two widgets share each of them: the counter
/// of the arrivals that wait, which the pill and the announcement read, and
/// the fact that a keyboard navigation just happened, which the screen writes
/// and the pane reads.
library;

import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';

part 'queue_freeze.g.dart';

/// Which arrivals wait outside the frozen order.
///
/// The pill over the top row counts them, the screen reader hears the same
/// number -- and every decision that reaches further than the cursor takes
/// them out of its reach: a request that has never been on the screen must not
/// leave the machine because a key covered "the group" (`docs/UX.md` 2.8,
/// 5.4).
@Riverpod(keepAlive: true)
class PendingArrivals extends _$PendingArrivals {
  @override
  Set<FlowId> build() => const <FlowId>{};

  /// Records the arrivals that are waiting.
  void report(Set<FlowId> waiting) {
    if (!setEquals(state, waiting)) {
      state = waiting;
    }
  }
}

/// A counter that grows every time somebody asks for the waiting arrivals.
///
/// The frozen view lives in the `State` of the pane, so the key that merges
/// (`Shift+J`) cannot reach it directly; it raises this counter and the pane
/// answers. The pill does the same thing with a callback, because it stands
/// inside the pane.
@Riverpod(keepAlive: true)
class QueueMergeRequest extends _$QueueMergeRequest {
  @override
  int build() => 0;

  /// Asks the pane to take the waiting arrivals in.
  void request() => state = state + 1;
}

/// A counter that grows with every keyboard navigation in the queue.
///
/// A timestamp would have to be compared against a clock, and the clock of a
/// widget test is not the wall clock. A counter is enough: the pane restarts
/// its own `HMotion.freezeAfterKey` timer whenever the number changes, and a
/// timer is what the test harness can move.
@Riverpod(keepAlive: true)
class QueueKeyboardNav extends _$QueueKeyboardNav {
  @override
  int build() => 0;

  /// Says that `J`, `K` or an arrow key just moved the cursor.
  void touch() => state = state + 1;
}
