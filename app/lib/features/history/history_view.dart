/// How a recorded flow reads in the table: its visual state, and the six
/// short strings the columns print.
///
/// Formatting only, no widgets, so a test can check the strings without a
/// tree and the table and the detail head cannot disagree.
library;

import '../../core/domain/domain.dart';
import '../../core/ui/flow_visual_state.dart';
import '../../core/ui/ui.dart';

/// The code the recorder raises for a filter it cannot read.
///
/// Registered in `daemon/crates/core-types/src/diagnostics/codes.rs`; the app
/// matches on it to anchor the message under the filter field instead of over
/// the table (`docs/UX.md` 4.4). It belongs in `DiagnosticCodes` next to the
/// other client-side codes; it lives here until that file is touched again.
const String historyFilterInvalidCode = 'RECORDER_002';

/// The visual state a recorded flow is drawn in.
///
/// Five rules run before the shared derivation of [FlowVisualState], in this
/// order, because the history mixes eight states in one column while the
/// queue only ever shows one:
///
/// 1. A held flow is held; it has no status and no decision yet.
/// 2. Passthrough traffic keeps its own hue whatever happened to it.
/// 3. A timeout is a timeout. Its 504 is the answer the proxy wrote itself,
///    not an upstream failure, and the same holds for the 403 of a block:
///    a decision is never overruled by the status it produced.
/// 4. `no_route` is an error even though it is written as a block: nothing
///    was refused, the target was never reachable.
/// 5. Otherwise a failed upstream or a 5xx answer is an error, and everything
///    left is what the queue would show, so that the same flow does not
///    change colour when it moves from one screen to the other.
///
/// Rule 5 is a deliberate deviation from the derivation table of
/// `backlog/sprint-2.md`, which maps every block to `blocked`: a block by a
/// rule is `autoRule` in the queue, and two screens that paint one flow two
/// ways cost more than the table gains (`backlog/CONVENTIONS.md` 4.13,
/// predictability).
HFlowState historyVisualState(Flow flow) {
  if (flow.isHeld) {
    return HFlowState.held;
  }
  if (flow.passthrough) {
    return HFlowState.passthroughLlm;
  }
  if (flow.decision == DecisionKind.timedOut) {
    return HFlowState.timedOut;
  }
  if (flow.blockReason == BlockReason.noRoute) {
    return HFlowState.error;
  }
  if (flow.decision == DecisionKind.block) {
    return flow.visualState;
  }
  if (flow.state == FlowState.failed ||
      flow.upstreamError != null ||
      flow.status >= 500) {
    return HFlowState.error;
  }
  return flow.visualState;
}

/// The word for a decision, taken from the state labels the design system
/// already names.
///
/// A decision is not a visual state: an allow whose upstream failed was
/// still an allow, and the row's colour says the rest.
HFlowState historyDecisionLabelState(DecisionKind decision) =>
    switch (decision) {
      DecisionKind.allow => HFlowState.allowed,
      DecisionKind.allowEdited => HFlowState.allowedEdited,
      DecisionKind.block => HFlowState.blocked,
      DecisionKind.timedOut => HFlowState.timedOut,
    };

/// What decided the flow, as the short word the rule column prints.
enum HistoryDecider {
  /// A rule matched.
  rule,

  /// A person decided.
  manual,

  /// The hold budget ran out.
  timeout,

  /// The flow passed through to the configured LLM endpoint.
  passthrough,

  /// Nobody has decided yet.
  pending,
}

/// Which of the five words the rule column shows for [flow].
HistoryDecider historyDecider(Flow flow) {
  if (flow.passthrough) {
    return HistoryDecider.passthrough;
  }
  if (flow.decision == DecisionKind.timedOut ||
      flow.decisionSource == DecisionSource.timeout) {
    return HistoryDecider.timeout;
  }
  if (flow.ruleId != null || flow.decisionSource == DecisionSource.rule) {
    return HistoryDecider.rule;
  }
  return flow.isDecided ? HistoryDecider.manual : HistoryDecider.pending;
}

/// The short form of a rule id for the rule chip: the first eight characters
/// of the UUID, which is what distinguishes two rules of one session.
String historyRuleShort(RuleId id) =>
    id.value.length <= 8 ? id.value : id.value.substring(0, 8);

/// [at] as `HH:mm:ss` in the local zone.
///
/// Seconds and no date: the column is 72 px wide and the date is the same for
/// every row of a session. The full timestamp stands in the detail head,
/// where it is part of the evidence (`backlog/CONVENTIONS.md` 4.13).
String formatHistoryTime(DateTime at) {
  final DateTime local = at.toLocal();
  return '${_two(local.hour)}:${_two(local.minute)}:${_two(local.second)}';
}

/// [at] as `YYYY-MM-DD HH:mm:ss` in the local zone, for the detail head.
String formatHistoryTimestamp(DateTime at) {
  final DateTime local = at.toLocal();
  return '${local.year.toString().padLeft(4, '0')}-${_two(local.month)}-'
      '${_two(local.day)} ${_two(local.hour)}:${_two(local.minute)}:'
      '${_two(local.second)}';
}

/// [at] as RFC 3339 with a zone offset, the form HAR 1.2 asks for.
String formatHistoryIso8601(DateTime at) {
  final DateTime utc = at.toUtc();
  final String micros = utc.millisecond.toString().padLeft(3, '0');
  return '${utc.year.toString().padLeft(4, '0')}-${_two(utc.month)}-'
      '${_two(utc.day)}T${_two(utc.hour)}:${_two(utc.minute)}:'
      '${_two(utc.second)}.${micros}Z';
}

String _two(int value) => value.toString().padLeft(2, '0');

/// [bytes] in the shortest form the 72 px size column can carry: `0`, `512`,
/// `2.1k`, `48k`, `1.2M`.
///
/// Decimal units, the same base the daemon logs in. Below ten the value keeps
/// one decimal, above it drops it: `9.9k` and `48k` are both four characters.
String formatHistoryCompactSize(int bytes) {
  if (bytes < 1000) {
    return '$bytes';
  }
  const List<String> units = <String>['k', 'M', 'G', 'T'];
  double value = bytes / 1000;
  int unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit++;
  }
  final String text = value >= 10
      ? value.round().toString()
      : value.toStringAsFixed(1);
  return '$text${units[unit]}';
}

/// Request and response size as `2.1k / 48k`, with [unknown] where the daemon
/// has not seen the response yet.
String formatHistorySizePair(Flow flow, {required String unknown}) {
  final String request = formatHistoryCompactSize(flow.requestSize);
  final bool hasResponse = flow.responseSize > 0 || _responseIsFinal(flow);
  final String response = hasResponse
      ? formatHistoryCompactSize(flow.responseSize)
      : unknown;
  return '$request / $response';
}

/// True once no further response bytes can arrive.
///
/// Only [FlowState.recorded] and [FlowState.failed]. `responded` means the
/// head came back and the body is still running in: response chunks keep
/// raising the size after it, so a zero there would be a claim about a
/// number nobody has yet (`backlog/CONVENTIONS.md` 4.13).
bool _responseIsFinal(Flow flow) =>
    flow.state == FlowState.recorded || flow.state == FlowState.failed;

/// True while the answer is still coming in.
bool historyResponseStreaming(Flow flow) =>
    flow.state == FlowState.forwarded || flow.state == FlowState.responded;

/// [flow]'s duration in whole milliseconds, or [unknown] while it runs.
String formatHistoryDuration(Flow flow, {required String unknown}) {
  final Duration? duration = flow.duration;
  return duration == null ? unknown : '${duration.inMilliseconds}';
}

/// [flow]'s status, or [unknown] while no answer has come back.
String formatHistoryStatus(Flow flow, {required String unknown}) =>
    flow.status == 0 ? unknown : '${flow.status}';
