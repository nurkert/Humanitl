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

  /// A session-wide diagnostic, for example a TLS refusal.
  const factory FlowEvent.diagnostic({
    required DateTime at,
    required Diagnostic diagnostic,
  }) = FlowEventDiagnostic;

  /// The rule set changed; clients reload it.
  const factory FlowEvent.rulesChanged({
    required DateTime at,
    required int revision,
  }) = FlowEventRulesChanged;

  /// The agent asked for something through `humanitl.internal/ask`.
  const factory FlowEvent.agentAsk({
    required DateTime at,
    required String askId,
    required String text,
    @Default('') String suggestedHost,
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
    FlowEventLagged() ||
    FlowEventDiagnostic() ||
    FlowEventRulesChanged() ||
    FlowEventAgentAsk() => null,
  };
}
