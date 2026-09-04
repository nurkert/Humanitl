/// The agent's requests from `humanitl.internal/ask` (HUM-073, ADR-014).
///
/// The agent has one channel out of the sandbox, and this is the only
/// direction on it that carries the agent's own words. A request is a request
/// and nothing else: it holds no flow, it decides nothing, and it never
/// becomes a rule on its own. It becomes a card in the queue, and the human
/// decides what happens next.
///
/// The list keeps every request the daemon sent and has not been dismissed
/// here — never a capped window. Dropping the oldest silently would break the
/// one promise the card makes: one card per request. The daemon limits how
/// many requests can arrive (ten per minute per session, `humanitl-proxy`
/// `meta.rs`), so the list stays short without this side pruning it.
library;

import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/flow_events.dart';

part 'agent_asks.g.dart';

/// One request of the agent, as the queue holds it.
///
/// A value of its own rather than the wire event: what the card needs is the
/// four fields below, and a screen that carried the event type would grow a
/// second reason to change whenever the contract does.
@immutable
class AgentAsk {
  /// Creates a request.
  const AgentAsk({
    required this.id,
    required this.at,
    required this.text,
    required this.suggestedHost,
    required this.suggestedPath,
  });

  /// What identifies this request; two requests with the same words are two.
  final String id;

  /// When the daemon took it.
  final DateTime at;

  /// The agent's own words, already stripped of line breaks, control
  /// characters and invisible characters by the daemon (`sanitize_note`).
  final String text;

  /// The host of the first URL in the text, or empty when there was none.
  ///
  /// A suggestion for the rule sheet, nothing more: the human confirms it, and
  /// it stays editable, because it comes out of text the agent wrote.
  final String suggestedHost;

  /// The path of that same URL, or empty when it named none.
  ///
  /// Empty is the case the sheet has to say out loud: a rule without a path
  /// covers every path and every method of the host, and the agent asked for
  /// one address.
  final String suggestedPath;

  /// Host and path together, as the card shows them under the button.
  ///
  /// Never shortened anywhere: a host cut on the right is a different host to
  /// a reading eye, and this line stands next to the button that opens a rule.
  String get suggestedTarget =>
      suggestedHost.isEmpty ? '' : '$suggestedHost$suggestedPath';

  @override
  bool operator ==(Object other) =>
      other is AgentAsk &&
      other.id == id &&
      other.at == at &&
      other.text == text &&
      other.suggestedHost == suggestedHost &&
      other.suggestedPath == suggestedPath;

  @override
  int get hashCode => Object.hash(id, at, text, suggestedHost, suggestedPath);
}

/// The agent's open requests, newest first.
@Riverpod(keepAlive: true)
class AgentAsks extends _$AgentAsks {
  @override
  List<AgentAsk> build() {
    // `fireImmediately` is what starts the stream; a listener alone does not
    // build the provider it listens to (same reasoning as in `flows.dart`).
    ref.listen(flowEventsProvider, (
      AsyncValue<FlowEvent>? previous,
      AsyncValue<FlowEvent> next,
    ) {
      next.whenData(_apply);
    }, fireImmediately: true);
    return const <AgentAsk>[];
  }

  /// Everything that is not a request belongs to a flow and is folded in
  /// `flows.dart`; this notifier looks at one variant and ignores the rest.
  void _apply(FlowEvent event) {
    if (event case FlowEventAgentAsk(
      :final DateTime at,
      :final String askId,
      :final String text,
      :final String suggestedHost,
      :final String suggestedPath,
    )) {
      // The same id twice is a repeat after a gap in the stream, not a second
      // request: it replaces the entry rather than adding one.
      state = <AgentAsk>[
        AgentAsk(
          id: askId,
          at: at,
          text: text,
          suggestedHost: suggestedHost,
          suggestedPath: suggestedPath,
        ),
        ...state.where((AgentAsk open) => open.id != askId),
      ];
    }
  }

  /// Takes the request with [askId] off the queue.
  ///
  /// Dismissing is the human's answer "seen, nothing to do". It is local to
  /// this client: the daemon holds no state about a request, because a
  /// request holds nothing up.
  void dismiss(String askId) {
    state = <AgentAsk>[
      for (final AgentAsk open in state)
        if (open.id != askId) open,
    ];
  }
}
