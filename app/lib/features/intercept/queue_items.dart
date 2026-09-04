/// What the queue draws, line by line (HUM-029).
///
/// The pane needs one flat list: `AnimatedList` counts rows, not trees. A
/// group with two or more requests contributes its header and, while it is
/// open, its rows; a lone request stays the plain row of HUM-020. Every item
/// carries a stable key, because the difference between two frames is computed
/// over keys and never over positions.
library;

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/foundation.dart';

import '../../core/domain/domain.dart';
import 'providers/held_groups.dart';

/// One line of the queue.
@immutable
sealed class QueueItem {
  const QueueItem();

  /// What identifies this line across two frames.
  String get key;
}

/// The header of a group of requests.
@immutable
class QueueGroupHeader extends QueueItem {
  /// Creates the header of [group].
  const QueueGroupHeader(this.group);

  /// The group this line stands for.
  final HeldGroup group;

  @override
  String get key => 'group:${group.apex}';
}

/// One request.
@immutable
class QueueFlowRow extends QueueItem {
  /// Creates the row of [flow] inside [group].
  const QueueFlowRow(this.flow, this.group, {this.grouped = false});

  /// The request this line stands for.
  final Flow flow;

  /// The group it belongs to; a range selection stays inside it.
  final HeldGroup group;

  /// True while the row stands under a header of its own.
  final bool grouped;

  @override
  String get key => 'flow:${flow.id.value}';
}

/// The lines of [groups], with [isOpen] deciding which groups show their rows.
List<QueueItem> queueItems(
  HeldGroups groups,
  bool Function(HeldGroup group) isOpen,
) {
  final List<QueueItem> items = <QueueItem>[];
  for (final HeldGroup group in groups.groups) {
    if (!group.isBurst) {
      // One held request, plus whatever rests there after a decision: no
      // header, because a header for a single line adds no answer.
      for (final Flow flow in group.rows) {
        items.add(QueueFlowRow(flow, group));
      }
      continue;
    }
    items.add(QueueGroupHeader(group));
    if (isOpen(group)) {
      for (final Flow flow in group.rows) {
        items.add(QueueFlowRow(flow, group, grouped: true));
      }
    }
  }
  return items;
}

/// The keys of [items], for the diff.
List<String> queueItemKeys(List<QueueItem> items) => <String>[
  for (final QueueItem item in items) item.key,
];
