/// One screen asking another to take over a flow.
///
/// The history hands a request that is still held to the queue, because the
/// queue is where it can be decided. A feature may not reach into another
/// feature to do that (ARCHITECTURE 5), and it should not: the shell composes
/// the sections, so the shell is what performs the handover. The provider is
/// the note between the two.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../domain/domain.dart';

/// The flow another screen asked the queue to show, or null.
final NotifierProvider<FlowHandoffNotifier, FlowId?> flowHandoffProvider =
    NotifierProvider<FlowHandoffNotifier, FlowId?>(FlowHandoffNotifier.new);

/// The notifier behind [flowHandoffProvider].
class FlowHandoffNotifier extends Notifier<FlowId?> {
  @override
  FlowId? build() => null;

  /// Asks for [id] to be shown in the queue.
  void request(FlowId id) => state = id;

  /// Marks the request as carried out. Called by whoever performed it, so a
  /// second listener does not perform it again.
  void clear() => state = null;
}
