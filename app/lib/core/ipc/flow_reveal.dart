/// One screen asking the history to show a finished flow.
///
/// A diagnostic card over the queue can name a flow that no queue holds: the
/// flow behind `LLM_005` is a passthrough, and a passthrough is never held
/// (CONVENTIONS 4.29). The only place that can show it is the history, which
/// fetches it with `GetFlow` from the recording. A feature may not reach into
/// another feature to ask for that (ARCHITECTURE 5); the shell switches the
/// section, and the history opens the flow. This provider is the note between
/// them, the mirror of `flowHandoffProvider`.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../domain/domain.dart';

/// The flow another screen asked the history to show, or null.
final NotifierProvider<FlowRevealNotifier, FlowId?> flowRevealProvider =
    NotifierProvider<FlowRevealNotifier, FlowId?>(FlowRevealNotifier.new);

/// The notifier behind [flowRevealProvider].
class FlowRevealNotifier extends Notifier<FlowId?> {
  @override
  FlowId? build() => null;

  /// Asks for [id] to be shown in the history.
  void request(FlowId id) => state = id;

  /// Marks the request as carried out. Called by the history once the flow is
  /// open, so a second listener does not open it again.
  void clear() => state = null;
}
