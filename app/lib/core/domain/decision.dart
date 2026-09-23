/// A decision about a held flow, mirror of `humanitl_core::Decision`.
library;

import 'package:freezed_annotation/freezed_annotation.dart';

import 'flow_state.dart';
import 'http.dart';

part 'decision.freezed.dart';

/// What the human (or a rule, or the clock) decided.
@freezed
sealed class Decision with _$Decision {
  /// Allowed unchanged.
  ///
  /// [acknowledgedFindings] are the findings the person saw and sends anyway
  /// ("Send anyway" in the findings pause), as indices into the findings of
  /// the flow in the order the daemon reported them (HUM-160). Holding the
  /// release valve acknowledges nothing: the daemon then counts every
  /// finding as unresolved.
  const factory Decision.allow({
    @Default(<int>[]) List<int> acknowledgedFindings,
  }) = DecisionAllow;

  /// Allowed after editing; carries the whole request including the body.
  const factory Decision.allowEdited({required EditedRequest request}) =
      DecisionAllowEdited;

  /// Blocked, with an optional note for the agent (ARCHITECTURE 8.2).
  const factory Decision.block({
    @Default(BlockReason.user) BlockReason reason,
    String? note,
  }) = DecisionBlock;

  /// The hold budget ran out.
  const factory Decision.timedOut() = DecisionTimedOut;

  const Decision._();

  /// The [DecisionKind] this decision reports as.
  DecisionKind get kind => switch (this) {
    DecisionAllow() => DecisionKind.allow,
    DecisionAllowEdited() => DecisionKind.allowEdited,
    DecisionBlock() => DecisionKind.block,
    DecisionTimedOut() => DecisionKind.timedOut,
  };
}
