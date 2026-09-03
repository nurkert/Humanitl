/// Typed identifiers, mirrors of the `Uuid` newtypes in `humanitl-core`.
///
/// Extension types cost nothing at runtime and stop a flow id from being
/// passed where a rule id belongs. The wire form is the UUIDv7 text.
library;

import 'package:json_annotation/json_annotation.dart';

/// Identifier of a flow (`FlowId` in Rust, `flow_id` on the wire).
extension type const FlowId(String value) {
  /// Sort key: UUIDv7 text sorts by time when compared as a string.
  int compareTo(FlowId other) => value.compareTo(other.value);
}

/// Identifier of a rule (`RuleId` in Rust, `rule_id` on the wire).
extension type const RuleId(String value) {}

/// Identifier of a session (`SessionId` in Rust, `session_id` on the wire).
extension type const SessionId(String value) {
  /// The first eight characters, for the status bar.
  String get short => value.length <= 8 ? value : value.substring(0, 8);
}

/// Identifier of a sandbox (`SandboxId` in Rust, `sandbox_id` on the wire).
extension type const SandboxId(String value) {}

/// JSON converter for [FlowId]; json_serializable cannot see through an
/// extension type on its own.
class FlowIdConverter implements JsonConverter<FlowId, String> {
  /// Creates the converter.
  const FlowIdConverter();

  @override
  FlowId fromJson(String json) => FlowId(json);

  @override
  String toJson(FlowId object) => object.value;
}

/// JSON converter for [RuleId].
class RuleIdConverter implements JsonConverter<RuleId, String> {
  /// Creates the converter.
  const RuleIdConverter();

  @override
  RuleId fromJson(String json) => RuleId(json);

  @override
  String toJson(RuleId object) => object.value;
}
