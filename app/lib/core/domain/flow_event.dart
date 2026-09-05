/// One event of the daemon's stream, mirror of `FlowEvent` in
/// `humanitl.proto`. Bodies never travel with an event.
library;

import 'package:freezed_annotation/freezed_annotation.dart';

import 'diagnostic.dart';
import 'flow.dart';
import 'flow_state.dart';
import 'http.dart';
import 'ids.dart';

part 'flow_event.freezed.dart';

/// An event of the flow stream. [at] is the daemon's timestamp.
@freezed
sealed class FlowEvent with _$FlowEvent {
  /// A new flow arrived.
  const factory FlowEvent.received({
    required DateTime at,
    required Flow flow,
    DomainInfo? domain,
  }) = FlowEventReceived;

  /// The detectors finished.
  const factory FlowEvent.analyzed({
    required DateTime at,
    required FlowId flowId,
    @Default(<Finding>[]) List<Finding> findings,
  }) = FlowEventAnalyzed;

  /// The flow waits for a decision.
  const factory FlowEvent.held({
    required DateTime at,
    required FlowId flowId,
    required DateTime deadline,
    @Default(0) int queueBytes,
    @Default(0) int queueCount,
  }) = FlowEventHeld;

  /// A decision was made.
  const factory FlowEvent.decided({
    required DateTime at,
    required FlowId flowId,
    required DecisionKind kind,
    DecisionSource? source,
    BlockReason? blockReason,
    RuleId? ruleId,
    @Default('') String note,
  }) = FlowEventDecided;

  /// The request went upstream.
  const factory FlowEvent.forwarded({
    required DateTime at,
    required FlowId flowId,
  }) = FlowEventForwarded;

  /// The response headers arrived.
  const factory FlowEvent.responseHeaders({
    required DateTime at,
    required FlowId flowId,
    required HttpResponseHead head,
    @Default(false) bool streaming,
  }) = FlowEventResponseHeaders;

  /// Progress of a response; a counter, no data.
  const factory FlowEvent.responseChunk({
    required DateTime at,
    required FlowId flowId,
    required int bytesSoFar,
  }) = FlowEventResponseChunk;

  /// The flow was persisted.
  const factory FlowEvent.recorded({
    required DateTime at,
    required FlowId flowId,
  }) = FlowEventRecorded;

  /// The hold budget ran out.
  const factory FlowEvent.timedOut({
    required DateTime at,
    required FlowId flowId,
  }) = FlowEventTimedOut;

  /// Events were lost; the client resyncs with `ListFlows(since)`.
  const factory FlowEvent.lagged({required DateTime at, required int dropped}) =
      FlowEventLagged;

  /// A diagnostic of the session, or of one flow when [flowId] is set.
  ///
  /// The daemon sends two shapes for the same thing: `diagnostic` (field 12)
  /// for a finding that belongs to no single request -- a handshake without
  /// SNI, for example -- and `flow_diagnostic` (field 16) for one that does,
  /// such as the refused handshake of a `CONNECT`. The variant keeps the
  /// distinction instead of flattening it: a finding that names its flow can
  /// be shown next to that flow, and one that names none still has to arrive
  /// somewhere.
  const factory FlowEvent.diagnostic({
    required DateTime at,
    required Diagnostic diagnostic,
    FlowId? flowId,
  }) = FlowEventDiagnostic;

  /// The rule set changed; clients reload it.
  const factory FlowEvent.rulesChanged({
    required DateTime at,
    required int revision,
  }) = FlowEventRulesChanged;

  /// The agent asked for something through `humanitl.internal/ask`.
  ///
  /// [suggestedHost] and [suggestedPath] come from the first URL the daemon
  /// found in [text]; both are empty when it found none. The path is what
  /// keeps a rule made from this request as narrow as the request was: without
  /// it the rule would open every path of the host (HUM-073).
  const factory FlowEvent.agentAsk({
    required DateTime at,
    required String askId,
    required String text,
    @Default('') String suggestedHost,
    @Default('') String suggestedPath,
  }) = FlowEventAgentAsk;

  /// An allowed request did not reach its target.
  const factory FlowEvent.failed({
    required DateTime at,
    required FlowId flowId,
    required UpstreamError error,
    @Default('') String resolvedIp,
  }) = FlowEventFailed;

  const FlowEvent._();

  /// The flow this event is about, or null for session-wide events.
  FlowId? get flowId => switch (this) {
    FlowEventReceived(:final flow) => flow.id,
    FlowEventAnalyzed(:final flowId) ||
    FlowEventHeld(:final flowId) ||
    FlowEventDecided(:final flowId) ||
    FlowEventForwarded(:final flowId) ||
    FlowEventResponseHeaders(:final flowId) ||
    FlowEventResponseChunk(:final flowId) ||
    FlowEventRecorded(:final flowId) ||
    FlowEventTimedOut(:final flowId) ||
    FlowEventFailed(:final flowId) => flowId,
    // Ein eigener Zweig und keine Oder-Verkettung: Die Variante bindet
    // `FlowId?`, die Gruppe darüber `FlowId`, und ein Oder-Muster verlangt in
    // jedem Zweig denselben Typ. Erreicht wird dieser Zweig zur Laufzeit nie,
    // weil das erzeugte Feld `flowId` der Variante diesen Getter überschreibt;
    // der Zweig steht hier, weil der Schalter erschöpfend sein muss, und er
    // liefert denselben Wert wie das Feld.
    FlowEventDiagnostic(:final FlowId? flowId) => flowId,
    FlowEventLagged() || FlowEventRulesChanged() || FlowEventAgentAsk() => null,
  };
}
