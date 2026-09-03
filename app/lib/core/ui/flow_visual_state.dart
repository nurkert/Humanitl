/// Which of the eight visual states (`HFlowState`, BACKLOG.md 5) a domain
/// `Flow` shows in a row or a card. Shared by the queue and, later, the
/// history: both are projections of the same stream.
library;

import '../domain/domain.dart';
import 'ui.dart';

/// The visual state of a [Flow].
extension FlowVisualState on Flow {
  /// The colour and glyph the flow is drawn with.
  HFlowState get visualState {
    if (passthrough) {
      return HFlowState.passthroughLlm;
    }
    if (state == FlowState.failed) {
      return HFlowState.error;
    }
    return switch (decision) {
      null => HFlowState.held,
      DecisionKind.timedOut => HFlowState.timedOut,
      DecisionKind.allowEdited => HFlowState.allowedEdited,
      DecisionKind.allow =>
        decisionSource == DecisionSource.rule
            ? HFlowState.autoRule
            : HFlowState.allowed,
      DecisionKind.block =>
        decisionSource == DecisionSource.rule
            ? HFlowState.autoRule
            : HFlowState.blocked,
    };
  }
}
