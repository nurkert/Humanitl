/// The bookkeeping `AnimatedList` needs: which rows were inserted, which were
/// removed, in an order that keeps every index valid while it is applied.
///
/// Diffing over ids instead of positions is what stops the "index out of
/// range" exceptions of a fast queue (HUM-020 Fallstricke).
library;

import 'package:flutter/foundation.dart';

/// What a [QueueEdit] does.
enum QueueEditKind {
  /// A row appeared at the index.
  insert,

  /// A row disappeared from the index.
  remove,
}

/// One edit of the rendered list.
@immutable
class QueueEdit {
  /// Creates an edit.
  const QueueEdit(this.kind, this.index);

  /// Whether the row appeared or disappeared.
  final QueueEditKind kind;

  /// Where, in the list as it is when the edit is applied.
  final int index;

  @override
  bool operator ==(Object other) =>
      other is QueueEdit && other.kind == kind && other.index == index;

  @override
  int get hashCode => Object.hash(kind, index);

  @override
  String toString() => '${kind.name}($index)';
}

/// The edits that turn [before] into [after], to be applied in order.
///
/// Removals come first, from the back, so that the indices of the ones still
/// to come do not move; insertions follow from the front. Rows that only
/// changed position produce no edit: the queue is ordered by deadline, and a
/// deadline does not move, so a reorder among held rows does not happen. The
/// list still ends with the right number of rows, which is what
/// `AnimatedList` insists on.
/// The keys are flow ids in a flat queue and item keys once the queue groups
/// (HUM-029); the function only ever compares them, so it takes whatever key
/// the caller can compare with `==`.
List<QueueEdit> listDiff<T>(List<T> before, List<T> after) {
  final Set<T> kept = after.toSet();
  final Set<T> known = before.toSet();
  final List<QueueEdit> edits = <QueueEdit>[];
  final List<T> working = List<T>.of(before);
  for (int i = working.length - 1; i >= 0; i--) {
    if (!kept.contains(working[i])) {
      edits.add(QueueEdit(QueueEditKind.remove, i));
      working.removeAt(i);
    }
  }
  for (int i = 0; i < after.length; i++) {
    if (!known.contains(after[i])) {
      edits.add(QueueEdit(QueueEditKind.insert, i));
      working.insert(i, after[i]);
    }
  }
  assert(
    working.length == after.length,
    'the diff has to end with as many rows as the queue has',
  );
  return edits;
}
