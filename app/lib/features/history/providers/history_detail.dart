/// What the detail half of the history screen shows: which row is selected,
/// and everything the daemon knows about it.
///
/// The list holds summaries only; the detail is fetched when a row is
/// selected and never before (`backlog/sprint-2.md`, HUM-032, Kontext). Die
/// aufgezeichneten Rümpfe kommen nicht von hier, sondern über
/// `flowBodyProvider` in `core`, denselben Provider, den die Warteschlange
/// liest (HUM-116).
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';

/// Which flow the detail half shows, or null.
final NotifierProvider<HistorySelectionNotifier, FlowId?>
historySelectionProvider = NotifierProvider<HistorySelectionNotifier, FlowId?>(
  HistorySelectionNotifier.new,
);

/// The notifier behind [historySelectionProvider].
class HistorySelectionNotifier extends Notifier<FlowId?> {
  @override
  FlowId? build() => null;

  /// Selects [id].
  void select(FlowId id) => state = id;

  /// Selects nothing.
  void clear() => state = null;
}

/// Everything the daemon knows about one recorded flow.
final historyDetailProvider = FutureProvider.autoDispose
    .family<FlowDetail, FlowId>(
      (Ref ref, FlowId id) => ref.watch(daemonClientProvider).getFlow(id),
    );
